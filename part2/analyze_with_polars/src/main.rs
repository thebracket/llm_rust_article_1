use std::fs::File;
use polars::prelude::*;
use anyhow::Result;

fn main() -> Result<()> {
    let mut df = CsvReadOptions::default()
        .with_has_header(true)
        .try_into_reader_with_file_path(Some("categories-wip.csv".into()))?
        .finish()?
        .group_by(["CATEGORY"])? // Group by categories
        .count()? // Count the number of rows in each group
        .sort( // Sort by domain count, descending
            ["DOMAIN_count"],
            SortMultipleOptions::default()
                .with_order_descending(true)
        )?;

    // Save a new CSV file with the results
    let mut output_file = File::create("category-count.csv")?;

    CsvWriter::new(&mut output_file)
        .include_header(true)
        .with_separator(b',')
        .finish(&mut df)?;

    Ok(())
}
