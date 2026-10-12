//! Repoint deal party links when parties merge.

use aether_sdk::events::Received;
use aether_sdk::prelude::*;

use crate::common::{id_of, Record};

fn repoint_field(rows: Vec<Record>, field: &str, into: &str) -> Result<u32> {
    let mut n = 0;
    for row in rows {
        let _: Option<Record> = db::update("deal", id_of(&row)?, &json!({ field: into }))?;
        n += 1;
    }
    Ok(n)
}

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
        return Ok(json!({ "contact_party": 0, "org_party": 0 }));
    }

    let contact = db::find::<Record>("deal").filter("contact_party", from).limit(500).all()?;
    let org = db::find::<Record>("deal").filter("org_party", from).limit(500).all()?;
    Ok(json!({
        "contact_party": repoint_field(contact, "contact_party", into)?,
        "org_party": repoint_field(org, "org_party", into)?,
    }))
}

handler! {
    fn on_party_merged(received: Received) -> Value { do_on_party_merged(received) }
}
