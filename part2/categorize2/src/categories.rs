
const KEYWORDS: [&str; 32] = [
    "Internet Service Provider",
    "Telecommunications",
    "Hosting",
    "Technology",
    "Education",
    "Government",
    "Banking/Finance",
    "Healthcare",
    "Cloud",
    "Energy",
    "Consulting",
    "Marketing",
    "Communications",
    "Business",
    "Media/Entertainment",
    "Travel",
    "News",
    "Gaming",
    "Logistics",
    "Automotive",
    "Retail",
    "Industry",
    "Sports",
    "Agriculture",
    "Fashion",
    "Infrastructure",
    "Community",
    "Pharmaceuticals",
    "Charity",
    "Adult",
    "Streaming",
    "Other",
];

pub fn category_prompt() -> String {
    let category_list = KEYWORDS.join(", ");
    format!("Categories MUST be one of the following: {category_list}")
}

pub fn word_in_list(word: &str) -> bool {
    KEYWORDS.contains(&word)
}