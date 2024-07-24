//! Reads the ASN data from an IPInfo CSV file, and returns a de-duplicated
//! list of domains.

use serde::Deserialize;
use anyhow::Result;
use itertools::Itertools;

#[derive(Deserialize)]
#[allow(dead_code)] // Ignore unused fields. They have to be here to match the CSV file.
struct AsnRow {
    start_ip: String,
    end_ip: String,
    asn: String,
    name: String,
    domain: String,
}

/// Load the ASN data from a CSV file, and return a list of domains.
pub fn load_asn_domains() -> Result<Vec<String>> {
    let data = include_str!("../../data/asn.csv");
    let mut reader = csv::Reader::from_reader(data.as_bytes());
    let rows: Vec<_> = reader
        .deserialize::<AsnRow>() // Deserialize - returns a result
        .into_iter() // Consume the iterator
        .flatten()// Keep only Ok records
        .map(|r| r.domain.to_lowercase().trim().to_string()) // Extract just the domain
        .filter(|d| !d.is_empty()) // Remove empty domains
        .sorted() // Sort the results
        .dedup() // Remove duplicates
        .collect(); // Move the results into a vector

    //println!("Loaded {} domains", rows.len());

    Ok(rows)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_asn_domains() {
        load_asn_domains().unwrap();
    }
}
