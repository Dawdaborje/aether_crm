//! Repoint customer.party when parties merge.

use aether_sdk::events::Received;
use aether_sdk::prelude::*;

use crate::common::{id_of, text, Record};

fn do_on_party_merged(received: Received) -> Result<Value> {
    let from = received
        .payload
        .get("from")
        .and_then(Value::as_str)
        .ok_or_else(|| Error::msg("party.merged needs from"))?;
    let into = received
        .payload
        .get("into")
        .and_then(Value::as_str)
        .ok_or_else(|| Error::msg("party.merged needs into"))?;
    if from == into {
        return Ok(json!({ "repointed": 0, "merged": 0 }));
    }

    let rows = db::find::<Record>("customer").filter("party", from).limit(500).all()?;
    let mut repointed = 0;
    let mut merged = 0;
    for row in rows {
        let id = id_of(&row)?.to_string();
        let company = text(&row, "company").unwrap_or("");
        if let Some(survivor) = db::find::<Record>("customer")
            .filter("party", into)
            .filter("company", company)
            .first()?
        {
            // Two overlays for the same company — keep the survivor, deactivate the loser.
            if id_of(&survivor)? != id {
                let _: Option<Record> = db::update("customer", &id, &json!({ "active": false, "party": into }))?;
                merged += 1;
            }
        } else {
            let _: Option<Record> = db::update("customer", &id, &json!({ "party": into }))?;
            repointed += 1;
        }
    }
    Ok(json!({ "repointed": repointed, "merged": merged }))
}

handler! {
    fn on_party_merged(received: Received) -> Value { do_on_party_merged(received) }
}
