use anyhow::{Context, Ok, Result};
use config::{Config, File};
use humantime_serde::re::humantime::format_duration;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, env, fmt::Display, fs, path::PathBuf, time::Duration};
use url::Url;

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct EnoughConfig {
    /// The default profile to use if none is specified
    pub default_profile: Option<String>,
    pub profiles: HashMap<String, Profile>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Profile {
    #[serde(with = "humantime_serde")]
    pub duration: Duration,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub websites: Vec<Url>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub apps: Vec<PathBuf>,
}

impl EnoughConfig {
    pub fn load(path: Option<PathBuf>) -> Result<Self> {
        let config_path = match path {
            Some(p) => p,
            None => Self::find_config_file().with_context(|| "No config file found")?,
        };

        if !config_path.exists() {
            anyhow::bail!(
                "Config file `{}` does not exist",
                config_path.to_string_lossy()
            );
        }

        let config = Config::builder()
            .add_source(File::from(config_path))
            .build()?
            .try_deserialize::<Self>()?;

        config.validate()?;
        Ok(config)
    }

    fn validate(&self) -> Result<()> {
        // validating that default_profile exists in the list of profiles
        if let Some(default_profile) = &self.default_profile {
            if !self.profiles.contains_key(default_profile) {
                anyhow::bail!(
                    "Default profile `{}` not found in profiles",
                    default_profile
                );
            }
        }

        for (profile_name, profile) in &self.profiles {
            for website in &profile.websites {
                Self::validate_website(website).with_context(|| {
                    format!(
                        "Invalid website URL `{}` in profile `{}`",
                        website, profile_name
                    )
                })?;
            }

            for app in &profile.apps {
                if !app.exists() {
                    anyhow::bail!(
                        "App path `{}` specified in profile `{}` does not exist",
                        app.display(),
                        profile_name
                    );
                }
            }
        }

        Ok(())
    }

    fn validate_website(url: &Url) -> Result<()> {
        if !matches!(url.scheme(), "http" | "https") {
            anyhow::bail!("URL scheme must be http or https, found `{}`", url.scheme());
        }

        if url.host().is_none() {
            anyhow::bail!("URL must have a valid host, found `{}`", url);
        }

        Ok(())
    }

    fn find_config_file() -> Option<PathBuf> {
        let home = env::var("HOME").ok()?;
        let possible_paths = [
            PathBuf::from("enough.yaml"),
            PathBuf::from(&home).join(".config/enough/enough.yaml"),
            PathBuf::from(&home).join(".config/enough.yaml"),
        ];

        for path in possible_paths {
            if path.exists() {
                return Some(path);
            }
        }

        None
    }

    fn default_config_path() -> PathBuf {
        let home = env::var("HOME")
            .with_context(|| "$HOME environment variable not set")
            .unwrap();

        PathBuf::from(home).join(".config/enough/enough.yaml")
    }

    pub fn generate_sample(output_path: Option<PathBuf>) -> Result<String> {
        let config_path = match output_path {
            Some(path) => path.to_path_buf(),
            None => Self::default_config_path(),
        };

        if let Some(parent) = config_path.parent() {
            fs::create_dir_all(parent)?;
        }

        let sample_config = Self {
            default_profile: Some("lock-in".to_string()),
            profiles: HashMap::from([
                (
                    "lock-in".to_string(),
                    Profile {
                        duration: Duration::from_secs(125),
                        websites: vec![
                            Url::parse("https://www.youtube.com")?,
                            Url::parse("https://reddit.com")?,
                        ],
                        apps: vec![
                            PathBuf::from("/Applications/CrossOver.app"),
                            PathBuf::from("/Applications/Steam.app"),
                            PathBuf::from(
                                "/nix/store/d2ap3myk8zyzgfi9c2p87in3mvljvbw4-spotify-1.2.64.408/Applications/Spotify.app",
                            ),
                        ],
                    },
                ),
                (
                    "wind-down".to_string(),
                    Profile {
                        duration: Duration::from_secs(30),
                        websites: vec![
                            Url::parse("https://www.youtube.com")?,
                            Url::parse("https://www.reddit.com")?,
                            Url::parse("https://www.github.com")?,
                        ],
                        apps: vec![],
                    },
                ),
            ]),
        };

        let yaml_content = serde_yml::to_string(&sample_config)?;
        fs::write(&config_path, &yaml_content)?;

        eprintln!(
            "Sample config file created at `{}`",
            config_path.to_string_lossy()
        );

        Ok(yaml_content)
    }
}

impl Display for EnoughConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let w_name = 20;
        let w_dur = 12;
        let w_web = 8;
        let w_app = 4;

        writeln!(
            f,
            "  {:<w_name$} {:<w_dur$} {:<w_web$} {:<w_app$} {}",
            "Name",
            "Duration",
            "Websites",
            "Apps",
            "",
            w_name = w_name,
            w_dur = w_dur,
            w_web = w_web,
            w_app = w_app,
        )?;
        write!(
            f,
            "  {:-<w_name$} {:-<w_dur$} {:-<w_web$} {:-<w_app$} {}",
            "",
            "",
            "",
            "",
            "",
            w_name = w_name,
            w_dur = w_dur,
            w_web = w_web,
            w_app = w_app,
        )?;
        for (name, profile) in &self.profiles {
            let is_default = if let Some(default_profile) = &self.default_profile {
                name == default_profile
            } else {
                false
            };
            let marker = if is_default { " (default)" } else { "" };
            write!(
                f,
                "\n• {:<w_name$} {:<w_dur$} {:<w_web$} {:<w_app$} {}",
                name,
                format!(
                    "{:<w_dur$}",
                    format_duration(profile.duration),
                    w_dur = w_dur
                ),
                format!("{:<w_web$}", profile.websites.len(), w_web = w_web),
                format!("{:<w_app$}", profile.apps.len(), w_app = w_app),
                marker,
                w_name = w_name,
                w_dur = w_dur,
                w_web = w_web,
                w_app = w_app,
            )?;
        }

        std::fmt::Result::Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    #[test]
    fn write_sample_conf() -> Result<()> {
        let conf = EnoughConfig::generate_sample(Some(PathBuf::from("test.yml")))?;
        let yaml = serde_yml::to_string(&conf).unwrap();
        println!("YAML Config:\n{}", yaml);

        thread::sleep(Duration::from_secs(3));
        fs::remove_file("test.yml")?;

        Ok(())
    }
}
