//! `GET /api/events/categories`: the fixed upstream category list.

use chrono::{DateTime, TimeZone, Utc};
use serde_json::{Map, Value};

use super::EventCategoryRecord;

const CATEGORY_I18N: &[(&str, &str)] = &[
    ("art", "Art & Culture"),
    ("causes", "Causes"),
    ("competition", "Competition"),
    ("education", "Education"),
    ("gambling", "Gambling"),
    ("gaming", "Gaming"),
    ("giveaway", "Giveaway"),
    ("health", "Health & Wellbeing"),
    ("hobbies", "Hobbies & Passions"),
    ("identity", "Identity & Language"),
    ("live", "Live Performances"),
    ("music", "Music"),
    ("networking", "Networking"),
    ("nft", "NFT"),
    ("other", "Other"),
    ("party", "Party"),
    ("play", "Play to Earn"),
    ("poap", "POAP"),
    ("religion", "Religion & Spirituality"),
    ("shopping", "Shopping"),
    ("social", "Social Activities"),
    ("sports", "Sports"),
    ("talks", "Talks & Presentations"),
    ("town", "Town Hall"),
    ("tv", "TV & Movies"),
];

pub fn list() -> Vec<EventCategoryRecord> {
    let epoch: DateTime<Utc> = Utc.timestamp_opt(0, 0).single().unwrap_or_else(Utc::now);
    CATEGORY_I18N
        .iter()
        .map(|(name, en)| {
            let mut i18n = Map::new();
            i18n.insert("en".to_string(), Value::String((*en).to_string()));
            EventCategoryRecord {
                name: (*name).to_string(),
                active: true,
                created_at: epoch,
                updated_at: epoch,
                i18n,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_category_list_is_the_upstream_set() {
        let list = super::list();
        assert_eq!(list.len(), 25);
        assert_eq!(list[0].name, "art");
        assert_eq!(list[0].i18n["en"], "Art & Culture");
        assert!(list.iter().all(|c| c.active));
    }
}
