#![no_std]
extern crate alloc;
use berm_lang::http::Client;

const CLIENT: Client<'static> = Client {
    name: "AnyAPI",
    headers: &[("X-API-Key", "{secret:ANYAPI_API_KEY}")],
};

crab_openapi::service! {
    spec: env!("OPENAPI_SPEC"),
    client: crate::CLIENT,
    tools: cfg(feature = "tools"),
    operations: {
        /// Facebook ad details
        facebook_ad_details: "POST /v1/run/facebook.ad_details",
        /// Facebook ad details full
        facebook_ad_details_full: "POST /v1/run/facebook.ad_details_full",
        /// Facebook ad transcript
        facebook_ad_transcript: "POST /v1/run/facebook.ad_transcript",
        /// Facebook ads search
        facebook_ads_search: "POST /v1/run/facebook.ads_search",
        /// Facebook comment replies
        facebook_comment_replies: "POST /v1/run/facebook.comment_replies",
        /// Facebook company ads
        #[tool(catalog = "meta-ads.library.advertiser")]
        facebook_company_ads: "POST /v1/run/facebook.company_ads",
        /// Facebook event details
        facebook_event_details: "POST /v1/run/facebook.event_details",
        /// Facebook events
        facebook_events: "POST /v1/run/facebook.events",
        /// Facebook events search
        facebook_events_search: "POST /v1/run/facebook.events_search",
        /// Facebook followers
        facebook_followers: "POST /v1/run/facebook.followers",
        /// Facebook group posts
        #[tool(catalog = "facebook.group.posts")]
        facebook_group_posts: "POST /v1/run/facebook.group_posts",
        /// Facebook marketplace
        facebook_marketplace: "POST /v1/run/facebook.marketplace",
        /// Facebook marketplace item
        facebook_marketplace_item: "POST /v1/run/facebook.marketplace_item",
        /// Facebook marketplace location search
        facebook_marketplace_location_search: "POST /v1/run/facebook.marketplace_location_search",
        /// Facebook page contact
        facebook_page_contact: "POST /v1/run/facebook.page_contact",
        /// Facebook photos
        facebook_photos: "POST /v1/run/facebook.photos",
        /// Facebook post
        #[tool(catalog = "facebook.post.detail")]
        facebook_post: "POST /v1/run/facebook.post",
        /// Facebook post comments
        facebook_post_comments: "POST /v1/run/facebook.post_comments",
        /// Facebook post transcript
        facebook_post_transcript: "POST /v1/run/facebook.post_transcript",
        /// Facebook profile
        facebook_profile: "POST /v1/run/facebook.profile",
        /// Facebook profile events
        facebook_profile_events: "POST /v1/run/facebook.profile_events",
        /// Facebook profile posts
        #[tool(catalog = "facebook.user.posts")]
        facebook_profile_posts: "POST /v1/run/facebook.profile_posts",
        /// Facebook profile reels
        facebook_profile_reels: "POST /v1/run/facebook.profile_reels",
        /// Facebook search companies
        facebook_search_companies: "POST /v1/run/facebook.search_companies",
        /// Facebook search pages
        facebook_search_pages: "POST /v1/run/facebook.search_pages",
        /// Facebook search posts
        facebook_search_posts: "POST /v1/run/facebook.search_posts",
        /// Google ai mode
        google_ai_mode: "POST /v1/run/google.ai_mode",
        /// Google ai overview
        #[tool(catalog = "google.serp.ai_overview")]
        google_ai_overview: "POST /v1/run/google.ai_overview",
        /// Google autocomplete
        #[tool(catalog = "google.keywords.ideas")]
        google_autocomplete: "POST /v1/run/google.autocomplete",
        /// Google images
        google_images: "POST /v1/run/google.images",
        /// Google lens
        google_lens: "POST /v1/run/google.lens",
        /// Google news
        #[tool(catalog = "google.serp.news")]
        google_news: "POST /v1/run/google.news",
        /// Google patents
        google_patents: "POST /v1/run/google.patents",
        /// Google scholar
        google_scholar: "POST /v1/run/google.scholar",
        /// Google search
        #[tool(catalog = "google.serp.organic")]
        google_search: "POST /v1/run/google.search",
        /// Google search 100
        google_search_100: "POST /v1/run/google.search_100",
        /// Google shopping search
        google_shopping_search: "POST /v1/run/google_shopping.search",
        /// Google videos
        google_videos: "POST /v1/run/google.videos",
        /// Instagram audio reels
        instagram_audio_reels: "POST /v1/run/instagram.audio_reels",
        /// Instagram basic profile
        instagram_basic_profile: "POST /v1/run/instagram.basic_profile",
        /// Instagram comment replies
        instagram_comment_replies: "POST /v1/run/instagram.comment_replies",
        /// Instagram embed
        instagram_embed: "POST /v1/run/instagram.embed",
        /// Instagram hashtag analytics
        instagram_hashtag_analytics: "POST /v1/run/instagram.hashtag_analytics",
        /// Instagram hashtag recent posts
        instagram_hashtag_recent_posts: "POST /v1/run/instagram.hashtag_recent_posts",
        /// Instagram hashtag top posts
        instagram_hashtag_top_posts: "POST /v1/run/instagram.hashtag_top_posts",
        /// Instagram highlight detail
        instagram_highlight_detail: "POST /v1/run/instagram.highlight_detail",
        /// Instagram location posts
        instagram_location_posts: "POST /v1/run/instagram.location_posts",
        /// Instagram media transcript
        instagram_media_transcript: "POST /v1/run/instagram.media_transcript",
        /// Instagram post
        #[tool(catalog = "instagram.post.detail")]
        instagram_post: "POST /v1/run/instagram.post",
        /// Instagram post comments
        #[tool(catalog = "instagram.post.comments")]
        instagram_post_comments: "POST /v1/run/instagram.post_comments",
        /// Instagram post likers
        instagram_post_likers: "POST /v1/run/instagram.post_likers",
        /// Instagram profile
        instagram_profile: "POST /v1/run/instagram.profile",
        /// Instagram profile contact
        instagram_profile_contact: "POST /v1/run/instagram.profile_contact",
        /// Instagram reel transcript
        instagram_reel_transcript: "POST /v1/run/instagram.reel_transcript",
        /// Instagram reels search
        #[tool(catalog = "instagram.search.reels")]
        instagram_reels_search: "POST /v1/run/instagram.reels_search",
        /// Instagram search
        instagram_search: "POST /v1/run/instagram.search",
        /// Instagram search hashtag
        #[tool(catalog = "instagram.hashtag.posts")]
        instagram_search_hashtag: "POST /v1/run/instagram.search_hashtag",
        /// Instagram search locations
        instagram_search_locations: "POST /v1/run/instagram.search_locations",
        /// Instagram search profiles
        instagram_search_profiles: "POST /v1/run/instagram.search_profiles",
        /// Instagram similar profiles
        instagram_similar_profiles: "POST /v1/run/instagram.similar_profiles",
        /// Instagram stories full
        instagram_stories_full: "POST /v1/run/instagram.stories_full",
        /// Instagram stories thin
        instagram_stories_thin: "POST /v1/run/instagram.stories_thin",
        /// Instagram tagged posts
        instagram_tagged_posts: "POST /v1/run/instagram.tagged_posts",
        /// Instagram trending reels
        instagram_trending_reels: "POST /v1/run/instagram.trending_reels",
        /// Instagram user highlights
        instagram_user_highlights: "POST /v1/run/instagram.user_highlights",
        /// Instagram user posts
        #[tool(catalog = "instagram.user.posts")]
        instagram_user_posts: "POST /v1/run/instagram.user_posts",
        /// Instagram user reels
        #[tool(catalog = "instagram.user.reels")]
        instagram_user_reels: "POST /v1/run/instagram.user_reels",
        /// Instagram user reposts
        instagram_user_reposts: "POST /v1/run/instagram.user_reposts",
        /// Linkedin ad
        linkedin_ad: "POST /v1/run/linkedin.ad",
        /// Linkedin ads
        linkedin_ads: "POST /v1/run/linkedin.ads",
        /// Linkedin ads search
        linkedin_ads_search: "POST /v1/run/linkedin.ads_search",
        /// Linkedin article
        linkedin_article: "POST /v1/run/linkedin.article",
        /// Linkedin company
        linkedin_company: "POST /v1/run/linkedin.company",
        /// Linkedin company employees
        #[tool(catalog = "linkedin.company.people")]
        linkedin_company_employees: "POST /v1/run/linkedin.company_employees",
        /// Linkedin company posts
        linkedin_company_posts: "POST /v1/run/linkedin.company_posts",
        /// Linkedin company posts thin
        linkedin_company_posts_thin: "POST /v1/run/linkedin.company_posts_thin",
        /// Linkedin company thin
        linkedin_company_thin: "POST /v1/run/linkedin.company_thin",
        /// Linkedin email
        #[tool(catalog = "person.email.find")]
        linkedin_email: "POST /v1/run/linkedin.email",
        /// Linkedin jobs
        #[tool(catalog = "linkedin.search.jobs")]
        linkedin_jobs: "POST /v1/run/linkedin.jobs",
        /// Linkedin jobs thin
        #[tool(catalog = "linkedin.search.jobs")]
        linkedin_jobs_thin: "POST /v1/run/linkedin.jobs_thin",
        /// Linkedin post
        linkedin_post: "POST /v1/run/linkedin.post",
        /// Linkedin post comments
        linkedin_post_comments: "POST /v1/run/linkedin.post_comments",
        /// Linkedin post reactions
        linkedin_post_reactions: "POST /v1/run/linkedin.post_reactions",
        /// Linkedin post transcript
        linkedin_post_transcript: "POST /v1/run/linkedin.post_transcript",
        /// Linkedin profile
        #[tool(catalog = "linkedin.user.profile")]
        linkedin_profile: "POST /v1/run/linkedin.profile",
        /// Linkedin profile posts full
        linkedin_profile_posts_full: "POST /v1/run/linkedin.profile_posts_full",
        /// Linkedin profile posts thin
        linkedin_profile_posts_thin: "POST /v1/run/linkedin.profile_posts_thin",
        /// Linkedin profile thin
        #[tool(catalog = "linkedin.user.profile")]
        linkedin_profile_thin: "POST /v1/run/linkedin.profile_thin",
        /// Linkedin search companies
        linkedin_search_companies: "POST /v1/run/linkedin.search_companies",
        /// Linkedin search posts
        #[tool(catalog = "linkedin.search.posts")]
        linkedin_search_posts: "POST /v1/run/linkedin.search_posts",
        /// Linkedin search posts full
        #[tool(catalog = "linkedin.search.posts")]
        linkedin_search_posts_full: "POST /v1/run/linkedin.search_posts_full",
        /// Linkedin search profiles
        linkedin_search_profiles: "POST /v1/run/linkedin.search_profiles",
        /// Linkedin search profiles email
        linkedin_search_profiles_email: "POST /v1/run/linkedin.search_profiles_email",
        /// Linkedin search profiles thin
        linkedin_search_profiles_thin: "POST /v1/run/linkedin.search_profiles_thin",
        /// Maps contacts
        maps_contacts: "POST /v1/run/maps.contacts",
        /// Maps place
        #[tool(catalog = "google.serp.maps")]
        maps_place: "POST /v1/run/maps.place",
        /// Maps reviews
        #[tool(catalog = "google.maps.reviews")]
        maps_reviews: "POST /v1/run/maps.reviews",
        /// Maps search
        #[tool(catalog = "google.serp.maps")]
        maps_search: "POST /v1/run/maps.search",
        /// Reddit post
        reddit_post: "POST /v1/run/reddit.post",
        /// Reddit post comments
        reddit_post_comments: "POST /v1/run/reddit.post_comments",
        /// Reddit post transcript
        reddit_post_transcript: "POST /v1/run/reddit.post_transcript",
        /// Reddit profile
        reddit_profile: "POST /v1/run/reddit.profile",
        /// Reddit search
        reddit_search: "POST /v1/run/reddit.search",
        /// Reddit subreddit details
        reddit_subreddit_details: "POST /v1/run/reddit.subreddit_details",
        /// Reddit subreddit posts
        reddit_subreddit_posts: "POST /v1/run/reddit.subreddit_posts",
        /// Reddit subreddit search
        reddit_subreddit_search: "POST /v1/run/reddit.subreddit_search",
        /// Reddit trending posts
        reddit_trending_posts: "POST /v1/run/reddit.trending_posts",
        /// Reddit user comments
        reddit_user_comments: "POST /v1/run/reddit.user_comments",
        /// Reddit user posts
        reddit_user_posts: "POST /v1/run/reddit.user_posts",
        /// Tiktok ad library ad
        tiktok_ad_library_ad: "POST /v1/run/tiktok.ad_library_ad",
        /// Tiktok ad library search
        tiktok_ad_library_search: "POST /v1/run/tiktok.ad_library_search",
        /// Tiktok ad transparency search
        tiktok_ad_transparency_search: "POST /v1/run/tiktok.ad_transparency_search",
        /// Tiktok audience demographics
        tiktok_audience_demographics: "POST /v1/run/tiktok.audience_demographics",
        /// Tiktok comment replies
        tiktok_comment_replies: "POST /v1/run/tiktok.comment_replies",
        /// Tiktok followers
        tiktok_followers: "POST /v1/run/tiktok.followers",
        /// Tiktok following
        tiktok_following: "POST /v1/run/tiktok.following",
        /// Tiktok hashtag videos
        tiktok_hashtag_videos: "POST /v1/run/tiktok.hashtag_videos",
        /// Tiktok live
        tiktok_live: "POST /v1/run/tiktok.live",
        /// Tiktok photos
        tiktok_photos: "POST /v1/run/tiktok.photos",
        /// Tiktok profile
        #[tool(catalog = "tiktok.user.profile")]
        tiktok_profile: "POST /v1/run/tiktok.profile",
        /// Tiktok profile contact
        tiktok_profile_contact: "POST /v1/run/tiktok.profile_contact",
        /// Tiktok profile region
        tiktok_profile_region: "POST /v1/run/tiktok.profile_region",
        /// Tiktok profile videos
        #[tool(catalog = "tiktok.user.videos")]
        tiktok_profile_videos: "POST /v1/run/tiktok.profile_videos",
        /// Tiktok search hashtag
        tiktok_search_hashtag: "POST /v1/run/tiktok.search_hashtag",
        /// Tiktok search keyword
        #[tool(catalog = "tiktok.search.videos")]
        tiktok_search_keyword: "POST /v1/run/tiktok.search_keyword",
        /// Tiktok search top
        #[tool(catalog = "tiktok.search.general")]
        tiktok_search_top: "POST /v1/run/tiktok.search_top",
        /// Tiktok search users
        #[tool(catalog = "tiktok.search.users")]
        tiktok_search_users: "POST /v1/run/tiktok.search_users",
        /// Tiktok shop categories
        tiktok_shop_categories: "POST /v1/run/tiktok_shop.categories",
        /// Tiktok shop creator
        tiktok_shop_creator: "POST /v1/run/tiktok_shop.creator",
        /// Tiktok shop product
        tiktok_shop_product: "POST /v1/run/tiktok_shop.product",
        /// Tiktok shop product reviews
        tiktok_shop_product_reviews: "POST /v1/run/tiktok_shop.product_reviews",
        /// Tiktok shop search
        tiktok_shop_search: "POST /v1/run/tiktok_shop.search",
        /// Tiktok shop search suggestions
        tiktok_shop_search_suggestions: "POST /v1/run/tiktok_shop.search_suggestions",
        /// Tiktok shop shop products
        tiktok_shop_shop_products: "POST /v1/run/tiktok_shop.shop_products",
        /// Tiktok shop user showcase
        tiktok_shop_user_showcase: "POST /v1/run/tiktok_shop.user_showcase",
        /// Tiktok song
        tiktok_song: "POST /v1/run/tiktok.song",
        /// Tiktok song videos
        tiktok_song_videos: "POST /v1/run/tiktok.song_videos",
        /// Tiktok top ads search
        tiktok_top_ads_search: "POST /v1/run/tiktok.top_ads_search",
        /// Tiktok trending feed
        tiktok_trending_feed: "POST /v1/run/tiktok.trending_feed",
        /// Tiktok trending hashtags
        #[tool(catalog = "tiktok.trends.hashtags")]
        tiktok_trending_hashtags: "POST /v1/run/tiktok.trending_hashtags",
        /// Tiktok video
        tiktok_video: "POST /v1/run/tiktok.video",
        /// Tiktok video comments
        #[tool(catalog = "tiktok.video.comments")]
        tiktok_video_comments: "POST /v1/run/tiktok.video_comments",
        /// Tiktok video download
        tiktok_video_download: "POST /v1/run/tiktok.video_download",
        /// Tiktok video transcript
        #[tool(catalog = "tiktok.video.captions")]
        tiktok_video_transcript: "POST /v1/run/tiktok.video_transcript",
        /// Tiktok video transcript full
        tiktok_video_transcript_full: "POST /v1/run/tiktok.video_transcript_full",
        /// Twitter article
        twitter_article: "POST /v1/run/twitter.article",
        /// Twitter community
        twitter_community: "POST /v1/run/twitter.community",
        /// Twitter community tweets
        twitter_community_tweets: "POST /v1/run/twitter.community_tweets",
        /// Twitter followers
        twitter_followers: "POST /v1/run/twitter.followers",
        /// Twitter following
        twitter_following: "POST /v1/run/twitter.following",
        /// Twitter profile
        #[tool(catalog = "x.user.profile")]
        twitter_profile: "POST /v1/run/twitter.profile",
        /// Twitter replies
        #[tool(catalog = "x.post.comments")]
        twitter_replies: "POST /v1/run/twitter.replies",
        /// Twitter search
        #[tool(catalog = "x.search.posts")]
        twitter_search: "POST /v1/run/twitter.search",
        /// Twitter search communities
        twitter_search_communities: "POST /v1/run/twitter.search_communities",
        /// Twitter search users
        twitter_search_users: "POST /v1/run/twitter.search_users",
        /// Twitter thread
        twitter_thread: "POST /v1/run/twitter.thread",
        /// Twitter trends
        twitter_trends: "POST /v1/run/twitter.trends",
        /// Twitter tweet
        twitter_tweet: "POST /v1/run/twitter.tweet",
        /// Twitter tweet transcript
        twitter_tweet_transcript: "POST /v1/run/twitter.tweet_transcript",
        /// Twitter user posts
        #[tool(catalog = "x.user.posts")]
        twitter_user_posts: "POST /v1/run/twitter.user_posts",
        /// Twitter user tweets
        #[tool(catalog = "x.user.posts")]
        twitter_user_tweets: "POST /v1/run/twitter.user_tweets",
        /// Web crawl
        #[tool(catalog = "web.crawl")]
        web_crawl: "POST /v1/run/web.crawl",
        /// Web map
        #[tool(catalog = "web.map")]
        web_map: "POST /v1/run/web.map",
        /// Web scrape
        #[tool(catalog = "web.extract")]
        web_scrape: "POST /v1/run/web.scrape",
        /// Web screenshot
        #[tool(catalog = "web.screenshot")]
        web_screenshot: "POST /v1/run/web.screenshot",
        /// Youtube channel
        youtube_channel: "POST /v1/run/youtube.channel",
        /// Youtube channel community posts
        youtube_channel_community_posts: "POST /v1/run/youtube.channel_community_posts",
        /// Youtube channel contact
        youtube_channel_contact: "POST /v1/run/youtube.channel_contact",
        /// Youtube channel lives
        youtube_channel_lives: "POST /v1/run/youtube.channel_lives",
        /// Youtube channel playlists
        youtube_channel_playlists: "POST /v1/run/youtube.channel_playlists",
        /// Youtube channel shorts
        youtube_channel_shorts: "POST /v1/run/youtube.channel_shorts",
        /// Youtube channel videos
        youtube_channel_videos: "POST /v1/run/youtube.channel_videos",
        /// Youtube comment replies
        youtube_comment_replies: "POST /v1/run/youtube.comment_replies",
        /// Youtube community post
        youtube_community_post: "POST /v1/run/youtube.community_post",
        /// Youtube playlist
        youtube_playlist: "POST /v1/run/youtube.playlist",
        /// Youtube search
        youtube_search: "POST /v1/run/youtube.search",
        /// Youtube search hashtag
        youtube_search_hashtag: "POST /v1/run/youtube.search_hashtag",
        /// Youtube search shorts
        youtube_search_shorts: "POST /v1/run/youtube.search_shorts",
        /// Youtube trending shorts
        youtube_trending_shorts: "POST /v1/run/youtube.trending_shorts",
        /// Youtube video
        youtube_video: "POST /v1/run/youtube.video",
        /// Youtube video comments
        youtube_video_comments: "POST /v1/run/youtube.video_comments",
        /// Youtube video sponsors
        youtube_video_sponsors: "POST /v1/run/youtube.video_sponsors",
        /// Youtube video transcript
        youtube_video_transcript: "POST /v1/run/youtube.video_transcript",
        /// Youtube video transcript full
        youtube_video_transcript_full: "POST /v1/run/youtube.video_transcript_full",
    },
}
