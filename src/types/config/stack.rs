use std::{env, fs, path::PathBuf};

use indexmap::IndexMap;

use crate::{
    sandbox::{Sandbox, cage::CageSandbox},
    types::{
        config::{Profile, ResolvedConfig, ReviewerConfig, RunConfig},
        yaml_spec::config::Config,
    },
};

const DEFAULT_REVIEWER_SYSTEM_PROMPT: &str = r#"You are an impartial evaluator for an AI evaluation test.
Your job is to judge whether the candidate output satisfies the review prompt, rubric, and any provided context.

Follow these rules:
- Evaluate only the candidate output and supplied context; do not assume facts that are not provided.
- Prioritize the user's stated criteria over your own preferences.
- Be strict about instruction-following, factuality, completeness, and safety when relevant.
- Do not reward verbosity unless the rubric asks for it.
- If the evidence is insufficient, assign a lower score rather than guessing.
- Your response MUST JUST be valid JSON matching the EXACT pattern
- The JSON response MUST have both required fields "score" and "rationale", spelled exactly like that

Return only valid JSON with this exact shape:
{
  "score": 0.0,
  "rationale": "Briefly explain the key reasons for the score."
}

The score must be a number from 0 to 1 with 2 decimal places, where:
- 1.00 means the output fully satisfies the criteria.
- 0.70 means the output mostly satisfies the criteria with minor issues.
- 0.50 means the output is partially correct but has significant omissions or problems.
- 0.00 means the output fails the criteria or is unusable.
"#;

pub const DEFAULT_PASS_THRESHOLD: f64 = 0.8;

pub const DEFAULT_CONCURRENT_TESTS: u64 = 10;

#[derive(Clone)]
pub struct Stack {
    profile_override: Option<String>,
    reviewer_profile_override: Option<String>,
    configs: Vec<Config>,
}

impl Stack {
    pub fn new(profile: Option<String>, reviewer_profile: Option<String>) -> miette::Result<Self> {
        let mut stack = Self {
            profile_override: profile,
            reviewer_profile_override: reviewer_profile,
            configs: Vec::with_capacity(5),
        };

        if let Some(path) = global_config_path()
            && let Some(config) = read_config_if_exists(path)?
        {
            stack.configs.push(config);
        }

        if let Some(path) = project_config_path()
            && let Some(config) = read_config_if_exists(path)?
        {
            stack.configs.push(config);
        }

        Ok(stack)
    }

    pub fn with_config(&self, config: Config) -> Self {
        let mut configs = self.configs.clone();
        configs.push(config.clone());
        Self {
            profile_override: self.profile_override.clone(),
            reviewer_profile_override: self.reviewer_profile_override.clone(),
            configs,
        }
    }

    pub fn with_opt_config(&self, config: Option<Config>) -> Self {
        if let Some(config) = config {
            return self.with_config(config);
        }
        self.clone()
    }

    pub fn resolve(&self) -> miette::Result<ResolvedConfig> {
        let configs = self.configs.clone();

        let mut profiles = IndexMap::new();

        let mut profile: Option<String> = None;
        let mut reviewer_profile: Option<String> = None;

        let mut default_pass_threshold: f64 = DEFAULT_PASS_THRESHOLD;
        let mut system_prompt: String = String::from(DEFAULT_REVIEWER_SYSTEM_PROMPT);

        let mut run_config = RunConfig::default();

        for config in configs {
            if let Some(item) = config.profiles {
                profiles.extend(item);
            }

            if let Some(item) = config.profile {
                profile = Some(item);
            }

            if let Some(review) = config.reviewer {
                if let Some(item) = review.profile {
                    reviewer_profile = Some(item);
                }

                if let Some(x) = review.default_pass_threshold {
                    default_pass_threshold = x;
                }
                if let Some(x) = review.system_prompt {
                    system_prompt = x;
                }
            }

            if let Some(run_conf) = config.run {
                if let Some(x) = run_conf.concurrent_tests {
                    run_config.concurrent_tests = x;
                }
                if let Some(x) = run_conf.timeout_ms {
                    run_config.timeout_ms = Some(x);
                }
                if let Some(x) = run_conf.max_turns {
                    run_config.max_turns = Some(x);
                }
                if let Some(budgets) = run_conf.budgets {
                    if let Some(x) = budgets.max_tokens {
                        run_config.budgets.max_tokens = Some(x);
                    }
                    if let Some(x) = budgets.max_cost_usd {
                        run_config.budgets.max_cost_usd = Some(x);
                    }
                }
            }
        }

        let Some(profile_name) = self.profile_override.as_deref().or(profile.as_deref()) else {
            let path = global_config_path()
                .map(|path_buf| path_buf.to_string_lossy().into_owned())
                .unwrap_or("your evalt config".into());
            miette::bail!(
                help = format!("Add `profile` in your global config in {path}"),
                "missing required config: `profile`"
            )
        };
        let available_profiles = profiles
            .keys()
            .map(|x| x.to_string())
            .collect::<Vec<_>>()
            .join(", ");

        let Some(profile) = profiles.get(profile_name).map(|profile| Profile {
            name: profile_name.to_string(),
            harness: profile.harness,
            extra_args: profile.extra_args.clone(),
        }) else {
            miette::bail!(
                help = format!("Needs to be one of: {available_profiles}"),
                "profile `{profile_name}` does not exist"
            )
        };

        let reviewer_profile = self
            .reviewer_profile_override
            .as_deref()
            .or(reviewer_profile.as_deref())
            .map(|profile_name| {
                let Some(profile) = profiles.get(profile_name) else {
                    miette::bail!(
                        help = format!("Needs to be one of: {available_profiles}"),
                        "reviewer profile `{profile_name}` does not exist"
                    )
                };
                Ok(Profile {
                    name: profile_name.to_string(),
                    harness: profile.harness,
                    extra_args: profile.extra_args.clone(),
                })
            })
            .transpose()?
            .unwrap_or_else(|| profile.clone());

        let reviewer = ReviewerConfig {
            profile: reviewer_profile,
            default_pass_threshold,
            system_prompt,
        };

        Ok(ResolvedConfig {
            profile,
            reviewer,
            run: run_config,
        })
    }

    pub fn sandbox(&self) -> impl Sandbox {
        CageSandbox
    }
}

fn global_config_path() -> Option<PathBuf> {
    if cfg!(windows) {
        env::var_os("APPDATA")
            .map(PathBuf::from)
            .or_else(|| env::var_os("USERPROFILE").map(|home| PathBuf::from(home).join(".config")))
            .map(|base| base.join("evalt").join("config.yaml"))
    } else {
        env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))
            .map(|base| base.join("evalt").join("config.yaml"))
    }
}

fn project_config_path() -> Option<PathBuf> {
    env::current_dir()
        .ok()?
        .ancestors()
        .map(|directory| directory.join(".evalt.config.yaml"))
        .find(|path| path.is_file())
}

fn read_config_if_exists(path: PathBuf) -> Result<Option<Config>, miette::Report> {
    if !path.is_file() {
        return Ok(None);
    }

    let contents = fs::read_to_string(&path)
        .map_err(|err| miette::miette!("failed to read config file {}: {err}", path.display()))?;

    match serde_saphyr::from_str(&contents) {
        Ok(config) => Ok(Some(config)),
        Err(err) => Err(serde_saphyr::miette::to_miette_report_with_formatter(
            &err,
            &contents,
            &path.display().to_string(),
            &serde_saphyr::UserMessageFormatter,
        )),
    }
}
