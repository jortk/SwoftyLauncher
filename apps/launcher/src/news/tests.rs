use super::feed::{Post, Trouble, embedded, format_date, image_format, merge, parse};
use super::state::{News, NewsScroll, StackedRow};

fn post(id: &str, date: &str) -> Post {
    Post {
        id: id.into(),
        kind: id.split('/').next().unwrap_or_default().into(),
        title: id.into(),
        date: date.into(),
        summary: String::new(),
        image: None,
        url: None,
        body: String::new(),
    }
}

#[test]
fn embeds_the_posts_in_full_and_the_release_notes_newest_first() {
    let posts = embedded();
    let news = posts.iter().find(|p| p.id == "news/1.3.0").unwrap();
    assert_eq!(news.kind, "news");
    assert!(
        news.body
            .starts_with("Swofty Launcher 1.3.0 makes everything an addon.")
    );
    assert!(news.body.contains("## Thank you, Team Simplified"));
    assert!(posts.windows(2).all(|w| w[0].date >= w[1].date));
    let notes = posts.iter().find(|p| p.id == "release/1.2.5").unwrap();
    assert_eq!(notes.title, "Swofty Launcher 1.2.5");
    assert_eq!(notes.version(), "1.2.5");
    assert_eq!(notes.url.as_deref(), Some("https://f1.swofty.net/releases/1.2.5"));
    assert!(!notes.summary.is_empty() && !notes.date.is_empty() && notes.body.is_empty());
}

#[test]
fn merges_by_id_and_sorts_by_date_then_version() {
    let mut base = vec![
        post("release/1.2.9", "2026-10-21"),
        post("release/1.2.10", "2026-10-21"),
        post("news/hello", "2026-10-21"),
    ];
    base[2].body = "kept".into();
    let mut newer = vec![
        post("release/1.2.9", "2026-10-21"),
        post("news/hello", "2026-10-21"),
    ];
    newer[0].summary = "fresh".into();
    let merged = merge(base, newer);
    let ids: Vec<&str> = merged.iter().map(|p| p.id.as_str()).collect();
    assert_eq!(ids, ["news/hello", "release/1.2.10", "release/1.2.9"]);
    assert_eq!(merged[2].summary, "fresh");
    assert_eq!(merged[0].body, "kept");
}

#[test]
fn reads_new_and_old_feeds() {
    let posts = parse(r#"{"posts":[{"id":"news/a","title":"A","extra":1}],"version":2}"#).unwrap();
    assert_eq!(posts[0].title, "A");
    assert!(posts[0].body.is_empty());
    let posts = parse(r###"{"posts":[{"id":"news/b","kind":"news","title":"B","body":"## Hi\n\n- one"}]}"###)
        .unwrap();
    assert_eq!(posts[0].body, "## Hi\n\n- one");
    assert!(parse("<html>").is_none());
}

#[test]
fn says_why_the_latest_news_is_missing() {
    let mut news = News::load(std::path::Path::new("/nonexistent"));
    assert_eq!(
        news.subtitle(),
        "Offline · showing the news that came with this version"
    );
    news.trouble = Some(Trouble::Failed);
    assert_eq!(
        news.subtitle(),
        "Couldn't load the latest news · showing the news that came with this version"
    );
    news.saved = true;
    news.trouble = Some(Trouble::Offline);
    assert_eq!(news.subtitle(), "Offline · showing the news saved last time");
    news.live = true;
    assert_eq!(news.subtitle(), "");
}

#[test]
fn formats_dates() {
    assert_eq!(format_date("2026-10-22"), "22 October 2026");
    assert_eq!(format_date("soon"), "soon");
}

#[test]
fn knows_image_formats() {
    assert!(image_format(b"\x89PNG\r\n").is_some());
    assert!(image_format(b"<svg").is_none());
}

#[test]
fn stacked_sync_tracks_banner_visibility() {
    let news = News::load(std::path::Path::new("/nonexistent"));
    let mut scroll = NewsScroll::default();

    scroll.sync(&news, false, true, true);
    let initial = scroll.stacked_rows.len();
    assert!(matches!(scroll.stacked_rows.first(), Some(StackedRow::Banner)));
    assert_eq!(scroll.stacked.item_count(), initial);

    scroll.sync(&news, false, true, false);
    assert_eq!(scroll.stacked_rows.len(), initial - 1);
    assert!(!matches!(scroll.stacked_rows.first(), Some(StackedRow::Banner)));
    assert_eq!(scroll.stacked.item_count(), initial - 1);

    scroll.sync(&news, false, true, true);
    assert_eq!(scroll.stacked_rows.len(), initial);
    assert!(matches!(scroll.stacked_rows.first(), Some(StackedRow::Banner)));
    assert_eq!(scroll.stacked.item_count(), initial);
}
