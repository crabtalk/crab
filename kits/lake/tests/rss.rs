//! The entries of RSS 2.0, RSS 1.0 and Atom feeds.

use crab_lake::rss;

/// 2026-10-09T23:27:00Z.
const NIGHT: u64 = 1_791_588_420_000;
/// 2026-10-11T06:55:00Z.
const MORNING: u64 = 1_791_701_700_000;

const RSS2: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<rss version="2.0" xmlns:dc="http://purl.org/dc/elements/1.1/"
     xmlns:content="http://purl.org/rss/1.0/modules/content/"
     xmlns:media="http://search.yahoo.com/mrss/">
  <channel>
    <title>The channel</title>
    <link>https://example.com/</link>
    <item>
      <title>First &amp; foremost</title>
      <link>https://example.com/1</link>
      <guid isPermaLink="false">post-1</guid>
      <dc:creator>Ada</dc:creator>
      <description><![CDATA[<p>A <b>summary</b></p>]]></description>
      <content:encoded>&lt;p&gt;The body&lt;/p&gt;</content:encoded>
      <pubDate>Fri, 09 Oct 2026 23:27:00 +0000</pubDate>
      <media:thumbnail url="https://example.com/1.jpg?a=1&amp;b=2"/>
      <enclosure url="https://example.com/1.png" type="image/png" length="1"/>
    </item>
    <item>
      <title>Second</title>
      <link>https://example.com/2</link>
      <guid>https://example.com/2</guid>
      <author>grace@example.com (Grace)</author>
      <enclosure url="https://example.com/2.mp3" type="audio/mpeg" length="1"/>
      <enclosure url="https://example.com/2.png" type="image/png" length="1"/>
    </item>
  </channel>
</rss>"#;

const RSS1: &str = r#"<?xml version="1.0"?>
<rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#"
         xmlns="http://purl.org/rss/1.0/"
         xmlns:dc="http://purl.org/dc/elements/1.1/">
  <channel rdf:about="https://example.org/">
    <title>An RDF channel</title>
  </channel>
  <item rdf:about="https://example.org/a">
    <title>A</title>
    <link>https://example.org/a</link>
    <description>About A</description>
    <dc:date>2026-10-11T06:55:00Z</dc:date>
    <dc:creator>Linus</dc:creator>
  </item>
</rdf:RDF>"#;

const ATOM: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<feed xmlns="http://www.w3.org/2005/Atom" xmlns:media="http://search.yahoo.com/mrss/">
  <title>An Atom feed</title>
  <updated>2026-10-12T00:00:00Z</updated>
  <entry>
    <id>tag:example.net,2026:1</id>
    <title type="html">One</title>
    <link rel="replies" href="https://example.net/1#comments"/>
    <link rel="alternate" type="text/html" href="https://example.net/1"/>
    <link href="https://example.net/1/other"/>
    <author><name>Margaret</name><email>m@example.net</email></author>
    <summary>Short</summary>
    <content type="html">&lt;p&gt;Long&lt;/p&gt;</content>
    <published>2026-10-11T08:55:00+02:00</published>
    <updated>2026-10-12T00:00:00Z</updated>
    <media:content url="https://example.net/1.mp4" medium="video"/>
    <media:content url="https://example.net/1.jpg" medium="image"/>
  </entry>
  <entry>
    <id>tag:example.net,2026:2</id>
    <title>Two</title>
    <link href="https://example.net/2"/>
    <updated>2026-10-09T23:27:00Z</updated>
  </entry>
</feed>"#;

#[test]
fn rss2_items_carry_what_their_children_say() {
    // A byte order mark and the whitespace after it are skipped.
    let entries = rss::entries(&format!("\u{feff}\n{RSS2}")).unwrap();
    assert_eq!(entries.len(), 2);

    let first = &entries[0];
    assert_eq!(first.id, "post-1");
    assert_eq!(first.title, "First & foremost");
    assert_eq!(first.link, "https://example.com/1");
    assert_eq!(first.author.as_deref(), Some("Ada"));
    assert_eq!(first.summary.as_deref(), Some("<p>A <b>summary</b></p>"));
    assert_eq!(first.content.as_deref(), Some("<p>The body</p>"));
    assert_eq!(first.posted, Some(NIGHT));
    assert_eq!(
        first.image.as_deref(),
        Some("https://example.com/1.jpg?a=1&b=2")
    );

    let second = &entries[1];
    assert_eq!(second.id, "https://example.com/2");
    assert_eq!(second.author.as_deref(), Some("grace@example.com (Grace)"));
    assert_eq!(second.summary, None);
    assert_eq!(second.posted, None);
    assert_eq!(second.image.as_deref(), Some("https://example.com/2.png"));
}

#[test]
fn rss1_items_are_identified_by_rdf_about() {
    let entries = rss::entries(RSS1).unwrap();
    assert_eq!(entries.len(), 1);
    let entry = &entries[0];
    assert_eq!(entry.id, "https://example.org/a");
    assert_eq!(entry.title, "A");
    assert_eq!(entry.link, "https://example.org/a");
    assert_eq!(entry.summary.as_deref(), Some("About A"));
    assert_eq!(entry.author.as_deref(), Some("Linus"));
    assert_eq!(entry.posted, Some(MORNING));
}

#[test]
fn atom_entries_take_the_alternate_link_and_fall_back_to_updated() {
    let entries = rss::entries(ATOM).unwrap();
    assert_eq!(entries.len(), 2);

    let one = &entries[0];
    assert_eq!(one.id, "tag:example.net,2026:1");
    assert_eq!(one.title, "One");
    assert_eq!(one.link, "https://example.net/1");
    assert_eq!(one.author.as_deref(), Some("Margaret"));
    assert_eq!(one.summary.as_deref(), Some("Short"));
    assert_eq!(one.content.as_deref(), Some("<p>Long</p>"));
    assert_eq!(one.posted, Some(MORNING));
    assert_eq!(one.image.as_deref(), Some("https://example.net/1.jpg"));

    let two = &entries[1];
    assert_eq!(two.link, "https://example.net/2");
    assert_eq!(two.author, None);
    assert_eq!(two.posted, Some(NIGHT));
}

#[test]
fn a_feed_with_no_items_has_no_entries() {
    let entries = rss::entries("<rss><channel><title>Empty</title></channel></rss>").unwrap();
    assert!(entries.is_empty());
}

#[test]
fn malformed_xml_is_an_error() {
    assert!(rss::entries("<rss><channel><item><title>Open</item").is_err());
}
