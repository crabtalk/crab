#![no_std]
extern crate alloc;
use berm_lang::http::Client;

pub mod news;

const CLIENT: Client<'static> = Client {
    name: "Litescrape",
    headers: &[("Authorization", "Bearer {secret:LITESCRAPE_API_KEY}")],
};

crab_openapi::service! {
    spec: env!("OPENAPI_SPEC"),
    client: crate::CLIENT,
    tools: cfg(feature = "tools"),
    operations: {
        apple_app_store_product: "GET /api/apple/app-store/product",
        apple_app_store_reviews: "GET /api/apple/app-store/reviews",
        apple_app_store_search: "GET /api/apple/app-store/search",
        apple_maps_places: "GET /api/apple/maps/places",
        apple_maps_reviews: "GET /api/apple/maps/reviews",
        bing_maps: "GET /api/bing/maps",
        bing_search: "GET /api/bing/search",
        duckduckgo_maps: "GET /api/duckduckgo/maps",
        duckduckgo_search: "GET /api/duckduckgo/search",
        google_ads: "GET /api/google/ads",
        google_ai_mode: "GET /api/google/ai-mode",
        google_ai_overview: "GET /api/google/ai-overview",
        google_contributor_reviews: "GET /api/google/contributor-reviews",
        google_local: "GET /api/google/local",
        google_maps: "GET /api/google/maps",
        google_maps_photo_meta: "GET /api/google/maps/photo-meta",
        google_maps_popular_times: "GET /api/google/maps/popular-times",
        google_maps_posts: "GET /api/google/maps/posts",
        google_play_apps: "GET /api/google/play/apps",
        google_play_books: "GET /api/google/play/books",
        google_play_games: "GET /api/google/play/games",
        google_play_movies: "GET /api/google/play/movies",
        google_play_product: "GET /api/google/play/product",
        google_play_reviews: "GET /api/google/play/reviews",
        google_reviews: "GET /api/google/reviews",
        google_search: "GET /api/google/search",
        google_shopping: "GET /api/google/shopping",
        google_shopping_product: "GET /api/google/shopping/product",
        tripadvisor_place: "GET /api/tripadvisor/place",
        tripadvisor_reviews: "GET /api/tripadvisor/reviews",
        tripadvisor_search: "GET /api/tripadvisor/search",
        web_fetch: "GET /api/web/fetch",
        web_fetch_post: "POST /api/web/fetch",
        web_screenshot: "GET /api/web/screenshot",
        web_screenshot_post: "POST /api/web/screenshot",
        yelp_reviews: "GET /api/yelp/reviews",
        yelp_search: "GET /api/yelp/search",
    },
}
