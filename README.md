# Categorizing Data with Large Language Models in Rust

LibreQoS is an open source project for monitoring and providing quality-of-experience for Internet Service Providers
(ISPs) and large networks. It runs as a "middle-box", monitoring traffic that passes through it. It recently
gained the ability to track individual *data flows* - connections between two endpoints.

Public Internet IP addresses belong to an ASN - an Autonomous System Number. An ASN is a unique number that identifies
a network on the Internet. Tracking flows allows you to see which ASNs your users are connecting to, how much data
is flowing, and monitor the quality of the connection.

Just providing ASN names isn't very useful. Most people who see that Joe is connecting to "SSI-AS" won't realize that 
this means they are watching Netflix! To make this data useful, we need to categorize the ASNs. There are a *lot* of 
ASN IP blocks - over 40,000! - so we need to automate this process.

## Obtaining the ASN Data

[ipinfo.io](https://ipinfo.io) provides downloadable CSV files containing information about IP addresses and ASNs. 
(An outdated copy is included in [data/asn.csv](data/asn.csv).) The data looks like this:

```csv
start_ip,end_ip,asn,name,domain
1.0.0.0,1.0.0.255,AS13335,"Cloudflare, Inc.",cloudflare.com
1.0.4.0,1.0.7.255,AS38803,Wirefreebroadband Pty Ltd,gtelecom.com.au
1.0.16.0,1.0.16.255,AS2519,ARTERIA Networks Corporation,arteria-net.com
```

> There are 420,772 lines in the file - we're not going to list them all here!

## Loading the Data

> The code for this is in [load_data/](./load_data/).

Rust makes reading CSV files easy. We'll use a couple of crates to help us out: Serde and CSV. 
We'll also include `anyhow` for easy error handling. You can add them as follows:

```bash
cargo add serde -F derive
cargo add csv
cargo add anyhow
```

Now we create a structure to define the data. We won't be using most of the fields, so we'll add a]
`#[allow(dead_code)]` to suppress warnings about unused fields. We also add `#[derive(Deserialize)]`
to automatically generate the code to read the data from the CSV file.

```rust
#[derive(Deserialize)]
#[allow(dead_code)] // Ignore unused fields. They have to be here to match the CSV file.
struct AsnRow {
    start_ip: String,
    end_ip: String,
    asn: String,
    name: String,
    domain: String,
}
```

Now we can read the data from the CSV file. We only care about the `domain` field, so we'll
write some code to load the data and return just that field:

```rust
pub fn load_asn_domains() -> Result<Vec<String>> {
    let data = include_str!("../../data/asn.csv");
    let mut reader = csv::Reader::from_reader(data.as_bytes());
    let rows: Vec<_> = reader
        .deserialize::<AsnRow>() // Deserialize - returns a result
        .into_iter() // Consume the iterator
        .flatten()// Keep only Ok records
        .map(|r| r.domain.to_lowercase().trim().to_string()) // Extract just the domain
        .filter(|d| !d.is_empty()) // Remove empty domains
        //.sorted() // Sort the results
        //.dedup() // Remove duplicates
        .collect(); // Move the results into a vector

    println!("Loaded {} domains", rows.len());

    Ok(rows)
}
```

Calling this function returns 412,795 domains. A scan through the data shows a *lot* of duplicates!
We don't want to categorize the same domain multiple times, so we need to de-duplicate the data.
Fortunately, a crate named `Itertools` makes this very easy.

> If you're tempted to just add it all to a `HashSet` and let that do the job---it'll work, but
> it will be substantially slower. Generating a hash for every string is expensive. It's *much*
> faster to sort the strings, iterate forward and only retain unique items!

```bash
cargo add itertools
```

Now we can add two lines to our function, and we're done!

```rust
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

    println!("Loaded {} domains", rows.len());

    Ok(rows)
}
```

That runs very quickly - and we're down to 63,519 domains. That's still a lot---but at least we aren't
doing the same work over and over again.

## Setting Up a Local LLM

You probably don't want to pay for 63,519 API calls (assuming everything is one shot, works perfectly
first time, and you never need to run a second test!). So lets set up a local LLM. I used
`Ollama` on my Linux box: it neatly wraps the complexities of `llama-cpp`, supports my AMD GPU
out of the box, and is easy to install. Your setup will vary by platform. Visit 
[https://ollama.com/](https://ollama.com/) and follow the instructions there. I'm using the
`llama3.1` model. I installed it with `ollama pull llama3.1`.

Once you have `Ollama` installed, you can test it with `ollama run llama3.1` and had a little
chat:

```
>>> Is Rust a great language?
Rust is a highly-regarded programming language that has gained popularity in recent years, and opinions about it vary depending on 
one's background, experience, and goals. Here are some aspects where Rust excels:

**Great features:**

1. **Memory Safety**: Rust's ownership model ensures memory safety without relying on garbage collection. This makes it an attractive 
choice for systems programming and applications that require high performance.
2. **Concurrency**: Rust provides built-in support for concurrency through its `async/await` syntax, making it easy to write 
asynchronous code.
3. **Performance**: Rust is designed with performance in mind. It can compete with C++ in terms of execution speed and memory usage.
4. **Type System**: Rust's type system is both expressive and flexible. It allows for static checking of types, ensuring that your 
code is correct at compile-time rather than runtime.

(and on, and on, and on - this is a chatty LLM!)
```

So now that we have a working local LLM, let's talk to it via the API from Rust.

## Talking to the LLM

In our Rust code, we're going to add two dependencies: Tokio (an async runtime) and 
Reqwest (an HTTP client). We'll also use `serde_json` to make JSON easy to work with.

```bash
cargo add tokio -F full
cargo add reqwest
cargo add serde_json -F json
```

Talking to Ollama uses a relatively simple [Rest API](https://github.com/ollama/ollama/blob/main/docs/api.md#generate-a-completion). If you've
taken our *Rust Foundations* or *Rust as a Service* classes, you'll know this one!

Let's start by setting a constant to the local LLM's API endpoint URL:

```rust
const LLM_API: &str = "http://localhost:11434/api/generate";
```

Now, we'll define a structure to receive data from the API:

```rust
#[derive(Deserialize)]
struct Response {
    response: String,
}
```

And finally, we can write a function that talks to the LLM:

```rust
async fn llm_completion(prompt: &str) -> Result<String> {
    // Use serde_json to quickly make a JSON object
    let request = json!({
        "model": "llama3.1",
        "prompt": prompt,
    });

    // Start the Reqwest client
    let client = reqwest::Client::new();
    // Create a POST request, add the request JSON, and send it
    let mut res = client.post(LLM_API)
        .json(&request)
        .send()
        .await?;

    // Empty string to assemble the response
    let mut response = String::new();
    
    // While res.chunk() returns Some(data), the stream
    // holds data we want. So we can grab each chunk,
    // and add it to the response string.
    while let Some(chunk) = res.chunk().await? {
        let chunk: Response = serde_json::from_slice(&chunk)?;
        response.push_str(&chunk.response);
    }

    // Return the response
    Ok(response)
}
```

The only tricky part here is that we are *streaming* the response, rather than
processing it all at once. LLMs return one response at a time. Fortunately,
streaming is built into Reqwest.

Let's give this a try:

```rust
#[tokio::main]
async fn main() -> Result<()> {
    let response = llm_completion("Good Morning!").await?;
    println!("{}", response);
    Ok(())
}
```

On my machine, this returns:

```
Good morning! Hope you're having a great start to the day! How can I help or chat with you today?
```

## Categorizing the Data: First Try - Oneshot!

"Oneshot" is a term used in the LLM world to describe a single request with no
helper data, no introspection, chain-of-thought or anything else. In a perfect
world, this would be all you need. (Note: this isn't a perfect world).

It will take a while to categorize 63,519 domains. We'll start with a small
sample. Let's add the `rand` crate (`cargo add rand`) to help us pick 
random test data.

We've already loaded the domain list, and we have a function to talk to the LLM---so 
let's put this together:

```rust
#[tokio::main]
async fn main() -> Result<()> {
    let domains = load_asn_domains()?;

    // Pick a small random sample
    let mut rng = rand::thread_rng();
    let sample = domains.choose_multiple(&mut rng, 2);

    // Let's do some categorizing
    for domain in sample {
        println!("Domain: {}", domain);
        let prompt = format!("Please categorize this domain with a single keyword. \
        Do not elaborate, do not explain or otherwise \
        enhance the answer. The domain is: {domain}");
        let response = llm_completion(&prompt).await?;
        println!("Response: {}", response);
    }
    Ok(())
}
```

Things to note:

* We're using a very simple prompt. Adding "do not elaborate" and "do not explain" is a
  common technique to get a single-word answer. LLMs like to talk.
* I like to say "please" to LLMs, so when the inevitable *Super Intelligence Apocalypse* happens, I'll be spared.
* We're not providing *any* additional information or context---we're relying on the LLM's baked-in knowledge.

Running this gave me:

```
Domain: r-tk.net
Response: Radio
Domain: 365it.fr
Response: Software
```

Is the answer any good? I'd not heard of either of those domains, so---unlike the LLM---I fired up
a browser to try and determine if the LLM was hallucinating (I was definitely expecting 
some hallucination!).

Unsurprisingly, the LLM was wrong. `r-tk.net` is a Russian company that provides Internet and
streaming television services. `365it.fr`is an IT services company in France.

I ran it a few more times, and the results weren't great. Sometimes, the LLM was
spot on---and most of the time, it was effectively random.

So let's try and give the LLM some context to work with.

## Adding Context

The vast majority of the listed domains have a website associated. Maybe we could
scrape text from the website and use that as context?

Let's make use of the `reqwest` crate to fetch the website data, and the `scraper`
crate to extract some text. LLMs have a pretty short context window, so we
don't want to overwhelm the poor AI with too much data.

```bash
cargo add scraper
```

We'll add a function to fetch the website data:

```rust
async fn website_text(domain: &str) -> Result<String> {
  let url = format!("http://{}/", domain);

  // Build a header with a Firefox user agent
  let mut headers = header::HeaderMap::new();
  headers.insert(
    header::USER_AGENT,
    header::HeaderValue::from_static("Mozilla/5.0 (platform; rv:geckoversion) Gecko/geckotrail Firefox/firefoxversion")
  );

  // Setup Reqwest with the header
  let client = reqwest::Client::builder()
          .default_headers(headers)
          .build()?;

  // Fetch the website
  let body = client
          .get(&url).send().await?
          .text().await?;

  // Parse the HTML
  let doc = scraper::Html::parse_document(&body);
  // Search for parts of the site with text in likely places
  let mut content = Vec::new();
  for items in ["title", "meta", "ul,li", "h1", "p"] {
    content.extend(find_content(items, &doc));
  }
  // We now have a big list of words (hopefully) from the website
  let result = content
          .into_iter() // Consuming iterator
          .sorted() // Sort alphabetically
          .dedup_with_count()// Deduplicatae, and return a tuple (count, word)
          .sorted_by(|a, b| b.0.cmp(&a.0)) // Sort by count, descending
          .map(|(count, word)| word)// Take only the word
          .take(100)// Take the top 100 words
          .join(" "); // Join them into a string

  Ok(result)
}
```

That's quite a mouthful, but it does a lot: it fetches the website, parses the HTML. It
then calls a helper function (`find_content`) to extract words from likely parts
of the site. It then sorts the words, deduplicates them, sorts them by count, and
returns the top 100 words as a string.

The helper function uses the `scraper` crate to extract text from the HTML,
and convert it into lowercase words as a vector of strings:

```rust
fn find_content(selector: &str, document: &Html) -> Vec<String> {
  let selector = scraper::Selector::parse(selector).unwrap();
  let mut content = Vec::new();
  for element in document.select(&selector) {
    // Get all text elements matching the selector
    let e: String = element.text().collect::<String>();

    // Split at whitespace, and filter out words shorter than 3 characters and
    // convert to lowercase.
    let e: Vec<String> = e.split_whitespace()
            .filter(|s| s.len() > 3)
            .map(|s| s.trim().to_lowercase())
            .collect();

    if !e.is_empty() {
      content.extend(e);
    }
  }

  content
}
```

Here's the resulting context from `provision.ro` (which my system picked at random):

```
security management data application threat protection risk firewall access endpoint digital privacy e-mail encryption gateway response secure testing detection hunting identity infrastructure network assessment training vulnerability advance analytics/ anti-phishing asset attacks authentication automated automation awareness bots browser casb centric classification client collaboration container cspm database ddos deception detection/protection discovery ediscovery governance human incident intelligence isolation malware mast penetration privilege rights runtime sase self-protection side siem soar third-party tools ueba visibility wireless media operations analysis cloud compliance generation masking messaging mobile next social trust zero about services solutions provision technologies partners contact find home more cyber experience expertise help information provision’s
```

So now we slightly change our `main` function to include the context in the prompt:

```rust
#[tokio::main]
async fn main() -> Result<()> {
    let domains = load_asn_domains()?;

    // Pick a small random sample
    let mut rng = rand::thread_rng();
    let sample = domains.choose_multiple(&mut rng, 5);

    // Let's do some categorizing
    for domain in sample {
        println!("Domain: {}", domain);
        if let Ok(text) = website_text(domain).await {
            let prompt = format!("Please categorize this domain with a single keyword. \
            Do not elaborate, do not explain or otherwise enhance the answer. \
            The domain is: {domain}. Here are some items from the website: {text}");

            let response = llm_completion(&prompt).await?;
            println!("Response: {}", response);
        } else {
            println!("unable to scrape: {domain}");
        }
    }
    Ok(())
}
```

So let's see how we're doing with some context included:

```
Domain: eternet.cc
Response: Internet
Domain: wilken-rz.de
unable to scrape: wilken-rz.de
Domain: orovalleyaz.gov
Response: Government
Domain: embl.de
Response: Biotechnology
Domain: baikonur.net
Response: Internet
```

Manually visiting these sites:

* `eternet.cc` is indeed an Internet provider.
* `orovalleyaz.gov` is the official site for the town of Oro Valley, Arizona. So "Government" is right.
* `embl.de` is the European Molecular Biology Laboratory. So "Biotechnology" is right.
* `baikonur.net` is a Russian site that provides Internet services. So "Internet" is right.

One scraping failure, and 4/4 on categorization! I repeated this a few times, and it was
consistently good!

## Let's Add Some Performance!

Running through each site one-at-a-time is going to take a *really long time*. Let's speed things up,
and make use of Tokio's async capabilities (one thread per core, work stealing). We'll add the
`futures` crate to provide `join_all`---which I find easier than Tokio's `JoinSet` system.

```bash
cargo add futures
```

### Appending Results to a File

Let's add a helper function that appends a line to a file:

```rust
async fn append_to_file(filename: &str, line: &str) -> Result<()> {
    let mut file = tokio::fs::OpenOptions::new()
        .append(true)
        .create(true)
        .open(filename)
        .await?;
    tokio::io::AsyncWriteExt::write_all(&mut file, format!("{}\n", line).as_bytes()).await?;
    Ok(())
}
```

This is *not* thread-safe---but it's designed to be called from a channel, which
will serialize the calls to it.

Next, we'll build a function that receives a report of failures, and uses the `append_to_file`
function to append errors as they occur:

```rust
async fn failures() -> Sender<String> {
    let (tx, mut rx) = tokio::sync::mpsc::channel::<String>(32);
    tokio::spawn(async move {
        while let Some(domain) = rx.recv().await {
            println!("Failed to scrape: {}", domain);
            // Append to "failures.txt"
            if let Err(e) = append_to_file("failures.txt", &domain).await {
                eprintln!("Failed to write to file: {}", e);
            }
        }
    });
    return tx;
}
```

We'll do the same for success, but we'll use a struct to store both the domain and the
category (I think a struct is nicer than a `(String, String)` tuple):

```rust
struct Domain {
    domain: String,
    category: String,
}

async fn success() -> Sender<Domain> {
    let (tx, mut rx) = tokio::sync::mpsc::channel::<Domain>(32);
    tokio::spawn(async move {
        while let Some(domain) = rx.recv().await {
            println!("Domain: {}, Category: {}", domain.domain, domain.category);
            // Append to "categories.csv"
            if let Err(e) = append_to_file("categories.csv", &format!("{},{}", domain.domain, domain.category)).await {
                eprintln!("Failed to write to file: {}", e);
            }
        }
    });
    return tx;
}
```

### Categorization as a Function

Let's move our prompt generation and calling into a function as well:

```rust
async fn categorize_domain(domain: &str, text: &str) -> Result<Domain> {
    let prompt = format!("Please categorize this domain with a single keyword in English. \
            Do not elaborate, do not explain or otherwise enhance the answer. \
            The domain is: {domain}. Here are some items from the website: {text}");

    let response = llm_completion(&prompt).await?;
    Ok(Domain {
        domain: domain.to_string(),
        category: response,
    })
}
```

### And Let's Call It!

We're probably going to melt some CPU and GPU chips here! Let's do it!

```rust
#[tokio::main]
async fn main() -> Result<()> {
  // Load the domains
  let mut domains = load_asn_domains()?;

  // Shuffle the domains (so in test runs we aren't always hitting the same ones)
  domains.shuffle(&mut rand::thread_rng());

  // Create the channels for results
  let report_success = success().await;
  let report_failures = failures().await;

  // Create a big set of tasks
  let already_done = std::fs::read_to_string("categories.csv").unwrap_or_default();
  let mut futures = Vec::new();
  for domain in domains.into_iter() {
    // Skip domains we've already done - in case we have to run it more than once
    if already_done.contains(&domain) {
      continue;
    }
    // Clone the channels - they are designed for this.
    let my_success = report_success.clone();
    let my_failure = report_failures.clone();
    let future = tokio::spawn(async move {
      match website_text(&domain).await {
        Ok(text) => {
          match categorize_domain(&domain, &text).await {
            Ok(domain) => { let _ = my_success.send(domain).await; },
            Err(_) => { let _ = my_failure.send(domain).await; },
          }
        }
        Err(_) => {
          let _ = my_failure.send(domain).await;
        }
      }
    });
    futures.push(future);

    // Limit the number of concurrent tasks
    if futures.len() >= 32 {
      let the_future: Vec<_> = futures.drain(..).collect();
      let _ = join_all(the_future).await;
    }
  }

  Ok(())
}
```

I ran this with `take(10)` to test it. There were no failures! The category list looks good, too:

```csv
phatriasulung.net.id,Webhosting
cornellcollege.edu,Education
connexcs.com,Communications
provedorsupply.net.br,Internet
thornburg.com,Investment
usp.org,Pharmaceuticals
valassis.com,Marketing
agen-rs.si,Energy
balasai.com,Hosting
lima.co.uk,Technology
```

We have the basis of a working categorization engine!

# Conclusion

LLMs provide a powerful tool for categorizing data, and Rust makes it easy to
work with them. Rust has excellent tools for scraping web data and massaging
the results, allowing you to provide context to your LLM calls. With Tokio, Reqwest
and futures, you can easily parallelize your work to make the most of your
hardware.

There's quite a few possible improvements to this code:

* Ask the LLM to use one of a provided list of keywords, rather than coming up with them for you.
* You could use more than one LLM and take the majority answer.
* You should try different LLM models. Llama 3 is a fun, open source model---but there's a *lot* of models out there!
* You could definitely improve the word selection algorithm. Remove stop words, prioritize the title, etc.
