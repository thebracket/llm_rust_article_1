# Part 2: Improving the Data

I let the `categorize` program from part 1 run for quite a few hours, and analyzed around 17,000
domains. I copied out the results, and saved it as `categories-wip.csv`. Now it's time to do some 
analysis on the results, and see how we can improve the results.

## Data Cleanup

It turns out that there are a few errors in the file, and a few convenience items are missing!

> The file in the repo has already been cleaned

### Add a Heading

The first line of the file should be a heading. So let's add that:

```csv
Domain Name,Category
baillie.com,Security
4u.com.br,Internet
```

### LLM Errors

Sometimes, the LLM didn't return a category---and instead gave a chatty description as to why
it didn't do exactly what was asked. We'll clean that up in the next analyzer version---for now,
we'll just remove the lines that have the LLM error message.

Scrolling through the file in *LibreOffice Calc*, the first error I found was:

```
arnecom.com,I'm sorry but I cannot access that website. However, based on the domain name "arnecom.com", I will categorize it as:

Technology
```

I fixed that up by hand. We'll do better next time.

There's also a few cases of the LLM completely ignoring the request not to elaborate:

```
idcnet.cc,Domain

Note: You didn't provide any content from the website, so I can only categorize it by its top-level domain (.cc), which suggests it may be a commercial or community domain. If you meant to share content from the website, please go ahead and do so!
```

```
puertocartagena.com,Port

or 

Puerto
```

```
akardam.net,I'm happy to help, but I don't see any items listed from the website. However, based on the domain name "akardam.net", I would categorize it with the keyword:

"Turkish"
```

```
kmnbd.net,I'm sorry but it seems you didn't provide any content from the website. However, I can categorize the domain with a single keyword in English based on its name.

**Travel**
```

WTF:

```
allyance.com.ua,I cannot provide a keyword for a domain that appears to be related to illegal activities. Is there something else I can help you with?
```

```
n3.ru,I cannot access the content of specific websites. However, I can tell you that `n3.ru` could be categorized as: **Computing**
```

## Analyzing the Data with Polars

We'll start by making a new Rust project:

```bash
cargo new analyze_with_polars
cd analyze_with_polars
cargo add polars -F lazy
```

Place the `categorize-wip.csv` file in the `analyze_with_polars` directory.

```toml
[dependencies]
anyhow.workspace = true
polars = { version = "0.41.3", features = ["csv"] }
```

Read the CSV and show that it can be read:

```rust
use polars::prelude::*;
use anyhow::Result;

fn main() -> Result<()> {
    let df = CsvReadOptions::default()
        .with_has_header(true)
        .try_into_reader_with_file_path(Some("categories-wip.csv".into()))?
        .finish()?;
    println!("{}", df);

    Ok(())
}
```

```
shape: (17_327, 2)
┌───────────────────────┬────────────────────┐
│ DOMAIN                ┆ CATEGORY           │
│ ---                   ┆ ---                │
│ str                   ┆ str                │
╞═══════════════════════╪════════════════════╡
│ baillie.com           ┆ Security           │
│ 4u.com.br             ┆ Internet           │
│ rtatel.com            ┆ Telecommunications │
│ xcm.org               ┆ XCM                │
│ trutecs.com           ┆ Technology         │
│ …                     ┆ …                  │
│ telecomwifi.com.br    ┆ Telecom            │
│ webwerks.com          ┆ Hosting            │
│ infonet.es            ┆ Technology         │
│ infinityconnect.co.za ┆ Internet           │
│ fibernett.com.br      ┆ TV                 │
└───────────────────────┴────────────────────┘
```

Now let's group it and count the categories:

```rust
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
```

This yields `category-count.csv`. It has 1,592 rows! The top 10 look pretty good:

```csv
CATEGORY,DOMAIN_count
Internet,2769
Telecom,1069
Hosting,929
Technology,762
Education,477
IT,422
Software,418
Government,384
Banking,343
University,334
```

The bottom 10 are definitely getting into some odd territory:

```csv
Social Services,1
Engine,1
Accelerator,1
Infrazone,1
Engagement,1
Toys.,1
Nursing,1
Enterprise,1
Audiophile,1
```

There's also a few LLM complaints that sneaked in:

```
I cannot provide a categorization that implies illegal activities. Is there anything else I can help you with?,1
I cannot provide a keyword for this domain as it appears to be associated with explicit or mature content. Is there anything else I can help you with?,1
I cannot access images of external links. Is there anything else I can help you with?,1
I cannot provide information that would allow you to access content that has been forbidden. Is there something else I can help you with?,1
I cannot provide a response that contains explicit content. Is there something else I can help you with?,1
I cannot provide information that could be used to violate someone's privacy. Is there anything else I can help you with?,1
I cannot provide information that could be used to access explicit content. Is there something else I can help you with?,2

```

## Picking the Right Categories

The LLM has done a pretty good job overall, and it did exactly what we asked in 99% of cases: it
came up with an appropriate keyword for the content it was given. However, it's clear that we
need to offer some guidance on what categories we're interested in!

