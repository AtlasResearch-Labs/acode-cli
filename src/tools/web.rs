//! Atlas Daedalus — Universal Web Intelligence Engine
//! Multi-source search, Reddit .json, arXiv research, YouTube transcripts, and Markdown extraction.

use std::time::Duration;
use reqwest::Client;
use regex::Regex;
use serde_json::Value;

pub struct WebTools {
    client: Client,
}

impl WebTools {
    pub fn new() -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(18))
            .connect_timeout(Duration::from_secs(8))
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36")
            .build()
            .unwrap_or_else(|_| Client::new());

        Self { client }
    }

    /// Multi-source web search (DuckDuckGo Lite + HackerNews Algolia fallback)
    pub async fn web_search(&self, query: &str, max_results: usize) -> Result<String, String> {
        let trimmed = query.trim();
        if trimmed.is_empty() {
            return Err("Search query cannot be empty.".to_string());
        }

        let limit = if max_results == 0 || max_results > 10 { 5 } else { max_results };

        // 1. If query is HackerNews-specific, query Algolia HN API directly
        if trimmed.to_lowercase().contains("hackernews") || trimmed.to_lowercase().contains("hacker news") || trimmed.starts_with("hn:") {
            let clean_q = trimmed.replace("hackernews", "").replace("hacker news", "").replace("hn:", "").trim().to_string();
            if let Ok(hn_res) = self.search_hackernews(&clean_q, limit).await {
                if !hn_res.is_empty() {
                    return Ok(hn_res);
                }
            }
        }

        // 2. DuckDuckGo HTML standard endpoint via POST
        let params = [("q", trimmed)];
        let resp = self.client.post("https://html.duckduckgo.com/html/")
            .form(&params)
            .header("Accept", "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8")
            .header("Accept-Language", "en-US,en;q=0.9")
            .header("Content-Type", "application/x-www-form-urlencoded")
            .send()
            .await;

        if let Ok(r) = resp {
            if r.status().is_success() {
                let html = r.text().await.unwrap_or_default();
                let results = Self::parse_ddg_html(&html, limit);
                if !results.is_empty() {
                    let mut out = format!("[WEB SEARCH RESULTS for \"{}\"]\n\n", trimmed);
                    for (i, (title, url, snippet)) in results.iter().enumerate() {
                        out.push_str(&format!("{}. [{}]({})\n   {}\n\n", i + 1, title, url, snippet));
                    }
                    return Ok(out.trim().to_string());
                }
            }
        }

        // 4. DuckDuckGo Instant Answer API fallback
        let api_url = format!("https://api.duckduckgo.com/?q={}&format=json&no_html=1&skip_disambig=1", Self::url_encode(trimmed));
        if let Ok(api_resp) = self.client.get(&api_url).send().await {
            if let Ok(json) = api_resp.json::<Value>().await {
                let mut out = format!("[WEB SEARCH RESULTS for \"{}\"]\n\n", trimmed);
                let mut found = false;

                if let Some(heading) = json.get("Heading").and_then(|h| h.as_str()) {
                    if let Some(abs) = json.get("Abstract").and_then(|a| a.as_str()) {
                        if !abs.is_empty() {
                            let url = json.get("AbstractURL").and_then(|u| u.as_str()).unwrap_or("");
                            out.push_str(&format!("1. [{}]({})\n   {}\n\n", heading, url, abs));
                            found = true;
                        }
                    }
                }

                if let Some(topics) = json.get("RelatedTopics").and_then(|t| t.as_array()) {
                    for t in topics.iter().take(limit) {
                        if let Some(text) = t.get("Text").and_then(|x| x.as_str()) {
                            let url = t.get("FirstURL").and_then(|u| u.as_str()).unwrap_or("");
                            out.push_str(&format!("- [{}]({})\n", text, url));
                            found = true;
                        }
                    }
                }

                if found {
                    return Ok(out.trim().to_string());
                }
            }
        }

        Err(format!("No search results found for: {}", trimmed))
    }

    /// HackerNews Algolia search
    async fn search_hackernews(&self, query: &str, limit: usize) -> Result<String, String> {
        let url = format!("https://hn.algolia.com/api/v1/search?query={}&tags=story&hitsPerPage={}", Self::url_encode(query), limit);
        let resp = self.client.get(&url).send().await.map_err(|e| e.to_string())?;
        let json: Value = resp.json().await.map_err(|e| e.to_string())?;

        let hits = json.get("hits").and_then(|h| h.as_array()).ok_or("No hits found")?;
        if hits.is_empty() {
            return Ok(String::new());
        }

        let mut out = format!("[HACKER NEWS SEARCH RESULTS for \"{}\"]\n\n", query);
        for (i, hit) in hits.iter().enumerate() {
            let title = hit.get("title").and_then(|t| t.as_str()).unwrap_or("Untitled");
            let target_url = hit.get("url").and_then(|u| u.as_str()).unwrap_or_else(|| {
                hit.get("story_url").and_then(|u| u.as_str()).unwrap_or("")
            });
            let object_id = hit.get("objectID").and_then(|id| id.as_str()).unwrap_or("");
            let points = hit.get("points").and_then(|p| p.as_u64()).unwrap_or(0);
            let comments = hit.get("num_comments").and_then(|c| c.as_u64()).unwrap_or(0);
            let hn_url = format!("https://news.ycombinator.com/item?id={}", object_id);

            let display_url = if target_url.is_empty() { &hn_url } else { target_url };
            out.push_str(&format!("{}. [{}]({})\n   Score: {} points | {} comments | Discussion: {}\n\n", i + 1, title, display_url, points, comments, hn_url));
        }

        Ok(out.trim().to_string())
    }

    /// Universal Web Fetcher with Smart Routing (Reddit, arXiv, YouTube, Direct, Jina Fallback)
    pub async fn web_fetch(&self, raw_url: &str, max_chars: usize) -> Result<String, String> {
        let url = raw_url.trim();
        if url.is_empty() {
            return Err("URL cannot be empty.".to_string());
        }

        let max_len = if max_chars == 0 { 8000 } else { max_chars };

        // 1. Reddit URL handler (native .json protocol)
        if url.contains("reddit.com/r/") {
            if let Ok(content) = self.fetch_reddit(url, max_len).await {
                if !content.is_empty() {
                    return Ok(content);
                }
            }
        }

        // 2. arXiv Research Paper handler (official export API)
        if url.contains("arxiv.org") {
            if let Ok(content) = self.fetch_arxiv(url).await {
                if !content.is_empty() {
                    return Ok(content);
                }
            }
        }

        // 3. YouTube Transcript handler
        if url.contains("youtube.com/watch") || url.contains("youtu.be/") {
            if let Ok(content) = self.fetch_youtube_transcript(url).await {
                if !content.is_empty() {
                    return Ok(content);
                }
            }
        }

        // 4. Standard Direct Fetch with Browser Identity
        match self.client.get(url)
            .header("Accept", "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8")
            .header("Accept-Language", "en-US,en;q=0.9")
            .header("Sec-Fetch-Dest", "document")
            .header("Sec-Fetch-Mode", "navigate")
            .header("Sec-Fetch-Site", "none")
            .send()
            .await
        {
            Ok(resp) => {
                let status = resp.status();
                if status.is_success() {
                    let html = resp.text().await.unwrap_or_default();
                    let cleaned = Self::sanitize_html_to_markdown(&html);
                    if cleaned.len() > 150 {
                        let truncated = if cleaned.len() > max_len {
                            format!("{}\n\n... [Content truncated at {} characters]", &cleaned[..max_len], max_len)
                        } else {
                            cleaned
                        };
                        return Ok(format!("[PAGE CONTENT: {}]\n\n{}", url, truncated));
                    }
                }
                // If 403, 429, or minimal body (SPA), fall through to Jina Reader
            }
            Err(_) => {
                // If connection error, fall through to Jina Reader
            }
        }

        // 5. Universal Jina Reader Fallback (Bypasses Cloudflare & Client-Side JS)
        let jina_url = format!("https://r.jina.ai/{}", url);
        let jina_resp = self.client.get(&jina_url)
            .header("Accept", "text/plain")
            .send()
            .await;

        if let Ok(jr) = jina_resp {
            if jr.status().is_success() {
                let markdown = jr.text().await.unwrap_or_default();
                if !markdown.trim().is_empty() {
                    let truncated = if markdown.len() > max_len {
                        format!("{}\n\n... [Content truncated at {} characters]", &markdown[..max_len], max_len)
                    } else {
                        markdown
                    };
                    return Ok(format!("[READER EXTRACT: {}]\n\n{}", url, truncated));
                }
            }
        }

        Err(format!("Failed to retrieve readable content from: {}", url))
    }

    /// Native Reddit .json Ingestion
    async fn fetch_reddit(&self, url: &str, max_len: usize) -> Result<String, String> {
        let clean_url = url.split('?').next().unwrap_or(url).trim_end_matches('/');
        let json_url = format!("{}.json?limit=8", clean_url);

        let resp = self.client.get(&json_url)
            .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) Daedalus/1.1.0")
            .send()
            .await
            .map_err(|e| e.to_string())?;

        let json: Value = resp.json().await.map_err(|e| e.to_string())?;

        let mut out = String::new();

        // Parse Post Details
        if let Some(post_listing) = json.get(0).and_then(|j| j.get("data")).and_then(|d| d.get("children")).and_then(|c| c.as_array()) {
            if let Some(post) = post_listing.first().and_then(|p| p.get("data")) {
                let title = post.get("title").and_then(|t| t.as_str()).unwrap_or("");
                let author = post.get("author").and_then(|a| a.as_str()).unwrap_or("unknown");
                let sub = post.get("subreddit_name_prefixed").and_then(|s| s.as_str()).unwrap_or("");
                let score = post.get("score").and_then(|s| s.as_i64()).unwrap_or(0);
                let selftext = post.get("selftext").and_then(|s| s.as_str()).unwrap_or("");

                out.push_str(&format!("# {} ({})\n", title, sub));
                out.push_str(&format!("Author: u/{} | Score: {} upvotes\n\n", author, score));
                if !selftext.is_empty() {
                    out.push_str(&format!("{}\n\n", selftext));
                }
                out.push_str("## Top Community Comments:\n\n");
            }
        }

        // Parse Comments
        if let Some(comment_listing) = json.get(1).and_then(|j| j.get("data")).and_then(|d| d.get("children")).and_then(|c| c.as_array()) {
            for (i, c) in comment_listing.iter().take(6).enumerate() {
                if let Some(comment) = c.get("data") {
                    let author = comment.get("author").and_then(|a| a.as_str()).unwrap_or("[deleted]");
                    let body = comment.get("body").and_then(|b| b.as_str()).unwrap_or("");
                    let score = comment.get("score").and_then(|s| s.as_i64()).unwrap_or(0);

                    if !body.is_empty() && author != "[deleted]" {
                        out.push_str(&format!("**Comment #{} by u/{} ({} pts):**\n{}\n\n", i + 1, author, score, body));
                    }
                }
            }
        }

        if out.len() > max_len {
            out.truncate(max_len);
            out.push_str("\n\n... [Reddit content truncated]");
        }

        Ok(out)
    }

    /// arXiv Research Paper Metadata and Abstract Extraction
    async fn fetch_arxiv(&self, url: &str) -> Result<String, String> {
        let re = Regex::new(r"(\d{4}\.\d{4,5})").unwrap();
        let id = match re.captures(url).and_then(|c| c.get(1)) {
            Some(m) => m.as_str(),
            None => return Err("Invalid arXiv ID in URL".to_string()),
        };

        let api_url = format!("https://export.arxiv.org/api/query?id_list={}", id);
        let resp = self.client.get(&api_url).send().await.map_err(|e| e.to_string())?;
        let xml = resp.text().await.unwrap_or_default();

        let entry_xml = if let Some(pos) = xml.find("<entry>") {
            &xml[pos..]
        } else {
            &xml
        };

        let title = Self::extract_xml_tag(entry_xml, "title").unwrap_or_else(|| "Unknown Title".to_string());
        let summary = Self::extract_xml_tag(entry_xml, "summary").unwrap_or_else(|| "No abstract available".to_string());
        let published = Self::extract_xml_tag(entry_xml, "published").unwrap_or_default();

        let re_author = Regex::new(r"<author>\s*<name>([^<]+)</name>").unwrap();
        let authors: Vec<String> = re_author.captures_iter(entry_xml).map(|c| c[1].trim().to_string()).collect();
        let author_line = if authors.is_empty() { String::new() } else { format!("Authors: {}\n", authors.join(", ")) };

        let clean_title = title.replace('\n', " ").trim().to_string();
        let clean_summary = summary.replace('\n', " ").trim().to_string();

        Ok(format!(
            "[ARXIV RESEARCH PAPER: {}]\n\n# {}\n{}Published: {}\nPDF URL: https://arxiv.org/pdf/{}.pdf\n\n## Abstract:\n{}\n",
            id, clean_title, author_line, published, id, clean_summary
        ))
    }

    /// YouTube Spoken Transcript Extraction
    async fn fetch_youtube_transcript(&self, url: &str) -> Result<String, String> {
        let re = Regex::new(r"(?:v=|\/)([a-zA-Z0-9_-]{11})").unwrap();
        let video_id = match re.captures(url).and_then(|c| c.get(1)) {
            Some(m) => m.as_str(),
            None => return Err("Invalid YouTube Video ID in URL".to_string()),
        };

        // Try direct timedtext track
        let timedtext_url = format!("https://www.youtube.com/api/timedtext?v={}&lang=en", video_id);
        if let Ok(resp) = self.client.get(&timedtext_url).send().await {
            if resp.status().is_success() {
                let xml = resp.text().await.unwrap_or_default();
                let clean_text = Self::clean_youtube_transcript_xml(&xml);
                if !clean_text.is_empty() {
                    return Ok(format!("[YOUTUBE SPOKEN TRANSCRIPT: {}]\n\n{}", video_id, clean_text));
                }
            }
        }

        // Fallback: Jina Reader handles YouTube URLs and extracts transcripts automatically
        let jina_yt = format!("https://r.jina.ai/https://www.youtube.com/watch?v={}", video_id);
        if let Ok(resp) = self.client.get(&jina_yt).send().await {
            if resp.status().is_success() {
                let text = resp.text().await.unwrap_or_default();
                if !text.is_empty() {
                    return Ok(format!("[YOUTUBE VIDEO TRANSCRIPT / SUMMARY: {}]\n\n{}", video_id, text));
                }
            }
        }

        Err(format!("Could not extract captions or transcript for YouTube video: {}", video_id))
    }

    // Helper: Parse DuckDuckGo Lite HTML results
    fn parse_ddg_lite_html(html: &str, limit: usize) -> Vec<(String, String, String)> {
        let mut results = Vec::new();
        let re_link = Regex::new(r#"<a class=['"]result-link['"]\s+href=['"]([^'"]+)['"]>([^<]+)</a>"#).unwrap();
        let re_snippet = Regex::new(r#"<td class=['"]result-snippet['"]>([^<]+)</td>"#).unwrap();

        let links: Vec<(String, String)> = re_link.captures_iter(html).map(|c| {
            let raw_url = c[1].to_string();
            let clean_url = Self::unwrap_ddg_redirect(&raw_url);
            let title = c[2].trim().to_string();
            (clean_url, title)
        }).collect();

        let snippets: Vec<String> = re_snippet.captures_iter(html).map(|c| c[1].trim().to_string()).collect();

        for (i, (url, title)) in links.into_iter().take(limit).enumerate() {
            let snip = snippets.get(i).cloned().unwrap_or_default();
            results.push((title, url, snip));
        }

        results
    }

    // Helper: Parse DuckDuckGo HTML results
    fn parse_ddg_html(html: &str, limit: usize) -> Vec<(String, String, String)> {
        let mut results = Vec::new();
        let re_link = Regex::new(r#"<a[^>]*class=['"][^'"]*result__a[^'"]*['"][^>]*href=['"]([^'"]+)['"][^>]*>([\s\S]*?)</a>"#).unwrap();
        let re_snippet = Regex::new(r#"<a[^>]*class=['"][^'"]*result__snippet[^'"]*['"][^>]*>([\s\S]*?)</a>"#).unwrap();

        let links: Vec<(String, String)> = re_link.captures_iter(html).map(|c| {
            let raw_url = c[1].to_string();
            let clean_url = Self::unwrap_ddg_redirect(&raw_url);
            let title = Self::strip_html_tags(&c[2]).trim().to_string();
            (clean_url, title)
        }).collect();

        let snippets: Vec<String> = re_snippet.captures_iter(html).map(|c| {
            Self::strip_html_tags(&c[1]).trim().to_string()
        }).collect();

        for (i, (url, title)) in links.into_iter().take(limit).enumerate() {
            let snip = snippets.get(i).cloned().unwrap_or_default();
            if !url.is_empty() && !title.is_empty() {
                results.push((title, url, snip));
            }
        }

        results
    }

    // Helper: Unwrap DDG redirect url (e.g. //duckduckgo.com/l/?uddg=https%3A%2F%2F...)
    fn unwrap_ddg_redirect(raw: &str) -> String {
        if let Some(pos) = raw.find("uddg=") {
            let substr = &raw[pos + 5..];
            let end_pos = substr.find('&').unwrap_or(substr.len());
            let encoded = &substr[..end_pos];
            let decoded = Self::url_decode(encoded);
            if !decoded.is_empty() {
                return decoded;
            }
        }
        raw.to_string()
    }

    fn url_encode(input: &str) -> String {
        let mut encoded = String::new();
        for byte in input.bytes() {
            match byte {
                b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                    encoded.push(byte as char);
                }
                b' ' => encoded.push('+'),
                _ => encoded.push_str(&format!("%{:02X}", byte)),
            }
        }
        encoded
    }

    fn url_decode(input: &str) -> String {
        let mut decoded = String::new();
        let mut chars = input.chars().peekable();
        while let Some(ch) = chars.next() {
            if ch == '%' {
                let h1 = chars.next();
                let h2 = chars.next();
                if let (Some(c1), Some(c2)) = (h1, h2) {
                    if let Ok(byte) = u8::from_str_radix(&format!("{}{}", c1, c2), 16) {
                        decoded.push(byte as char);
                        continue;
                    }
                }
            } else if ch == '+' {
                decoded.push(' ');
            } else {
                decoded.push(ch);
            }
        }
        decoded
    }

    // Helper: Convert raw HTML to readable Markdown
    fn sanitize_html_to_markdown(html: &str) -> String {
        let mut s = html.to_string();

        // Remove <script>, <style>, <svg>, <nav>, <footer>, <header>
        let drop_tags = [
            r"(?is)<script[^>]*>.*?</script>",
            r"(?is)<style[^>]*>.*?</style>",
            r"(?is)<svg[^>]*>.*?</svg>",
            r"(?is)<noscript[^>]*>.*?</noscript>",
            r"(?is)<nav[^>]*>.*?</nav>",
            r"(?is)<footer[^>]*>.*?</footer>",
            r"(?is)<header[^>]*>.*?</header>",
        ];
        for dt in drop_tags {
            if let Ok(re) = Regex::new(dt) {
                s = re.replace_all(&s, "").to_string();
            }
        }

        // Convert headings
        if let Ok(re) = Regex::new(r"(?i)<h1[^>]*>([\s\S]*?)</h1>") { s = re.replace_all(&s, "\n# $1\n").to_string(); }
        if let Ok(re) = Regex::new(r"(?i)<h2[^>]*>([\s\S]*?)</h2>") { s = re.replace_all(&s, "\n## $1\n").to_string(); }
        if let Ok(re) = Regex::new(r"(?i)<h3[^>]*>([\s\S]*?)</h3>") { s = re.replace_all(&s, "\n### $1\n").to_string(); }
        if let Ok(re) = Regex::new(r"(?i)<li[^>]*>([\s\S]*?)</li>") { s = re.replace_all(&s, "\n* $1").to_string(); }
        if let Ok(re) = Regex::new(r"(?i)<p[^>]*>([\s\S]*?)</p>") { s = re.replace_all(&s, "\n$1\n").to_string(); }
        if let Ok(re) = Regex::new(r"(?i)<br\s*/?>") { s = re.replace_all(&s, "\n").to_string(); }

        // Strip remaining HTML tags
        s = Self::strip_html_tags(&s);

        // Decode common HTML entities
        s = s.replace("&amp;", "&")
            .replace("&lt;", "<")
            .replace("&gt;", ">")
            .replace("&quot;", "\"")
            .replace("&#39;", "'")
            .replace("&nbsp;", " ");

        // Collapse multiple blank lines
        if let Ok(re) = Regex::new(r"\n{3,}") {
            s = re.replace_all(&s, "\n\n").to_string();
        }

        s.trim().to_string()
    }

    fn strip_html_tags(s: &str) -> String {
        if let Ok(re) = Regex::new(r"<[^>]*>") {
            re.replace_all(s, "").to_string()
        } else {
            s.to_string()
        }
    }

    fn extract_xml_tag(xml: &str, tag: &str) -> Option<String> {
        let pattern = format!(r"(?s)<{}>([\s\S]*?)</{}>", tag, tag);
        Regex::new(&pattern).ok().and_then(|re| {
            re.captures(xml).and_then(|c| c.get(1)).map(|m| m.as_str().to_string())
        })
    }

    fn clean_youtube_transcript_xml(xml: &str) -> String {
        let re = Regex::new(r"<text[^>]*>([\s\S]*?)</text>").unwrap();
        let mut out = Vec::new();
        for cap in re.captures_iter(xml) {
            let line = Self::strip_html_tags(&cap[1]);
            let decoded = line.replace("&amp;", "&").replace("&#39;", "'").replace("&quot;", "\"");
            if !decoded.trim().is_empty() {
                out.push(decoded.trim().to_string());
            }
        }
        out.join(" ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_url_encode_decode() {
        let original = "rust reqwest & tokio = awesome!";
        let encoded = WebTools::url_encode(original);
        assert!(encoded.contains("%26"));
        assert!(encoded.contains('+'));
        let decoded = WebTools::url_decode(&encoded);
        assert_eq!(decoded, original);
    }

    #[test]
    fn test_unwrap_ddg_redirect() {
        let raw = "//duckduckgo.com/l/?uddg=https%3A%2F%2Fcrates.io%2Fcrates%2Freqwest&rut=1";
        let unwrapped = WebTools::unwrap_ddg_redirect(raw);
        assert_eq!(unwrapped, "https://crates.io/crates/reqwest");
    }

    #[test]
    fn test_sanitize_html_to_markdown() {
        let raw_html = "<html><head><script>alert('noise')</script><style>.ad{color:red}</style></head><body><h1>Guide</h1><p>Learn <b>Rust</b> &amp; enjoy.</p></body></html>";
        let md = WebTools::sanitize_html_to_markdown(raw_html);
        assert!(!md.contains("alert"));
        assert!(!md.contains(".ad"));
        assert!(md.contains("# Guide"));
        assert!(md.contains("Learn Rust & enjoy."));
    }
}

