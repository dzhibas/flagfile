use chrono::NaiveDate;
use flagfile_lib::parse_flagfile::FlagDefinition;

use super::LintWarning;

pub fn check(name: &str, def: &FlagDefinition, today: NaiveDate) -> Vec<LintWarning> {
    let mut warnings = Vec::new();
    if let Some(expires) = def.metadata.expires {
        if expires < today {
            let days_ago = (today - expires).num_days();
            warnings.push(LintWarning::error(format!(
                "{} expired {} ({} days ago). Run: ff find -s {}",
                name, expires, days_ago, name
            )));
        }
    }
    warnings
}
