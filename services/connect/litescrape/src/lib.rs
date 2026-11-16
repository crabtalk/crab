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
        /// Apple app store product
        #[tool(catalog = "app-store.product")]
        apple_app_store_product: "GET /api/apple/app-store/product",
        /// Apple app store reviews
        #[tool(catalog = "app-store.reviews")]
        apple_app_store_reviews: "GET /api/apple/app-store/reviews",
        /// Apple app store search
        #[tool(catalog = "app-store.search.apps")]
        apple_app_store_search: "GET /api/apple/app-store/search",
        /// Apple maps places
        #[tool(catalog = "apple-maps.places")]
        apple_maps_places: "GET /api/apple/maps/places",
        /// Apple maps reviews
        #[tool(catalog = "apple-maps.reviews")]
        apple_maps_reviews: "GET /api/apple/maps/reviews",
        /// Bing maps
        #[tool(catalog = "bing.maps.search")]
        bing_maps: "GET /api/bing/maps",
        /// Bing search
        #[tool(catalog = "bing.serp.organic")]
        bing_search: "GET /api/bing/search",
        /// Duckduckgo maps
        #[tool(catalog = "duckduckgo.maps.search")]
        duckduckgo_maps: "GET /api/duckduckgo/maps",
        /// Duckduckgo search
        #[tool(catalog = "duckduckgo.serp.organic")]
        duckduckgo_search: "GET /api/duckduckgo/search",
        /// Google ads
        #[tool(catalog = "google.serp.ads")]
        google_ads: "GET /api/google/ads",
        /// Google ai mode
        #[tool(catalog = "google.serp.ai_mode")]
        google_ai_mode: "GET /api/google/ai-mode",
        /// Google ai overview
        #[tool(catalog = "google.serp.ai_overview")]
        google_ai_overview: "GET /api/google/ai-overview",
        /// Google contributor reviews
        #[tool(catalog = "google.maps.contributor_reviews")]
        google_contributor_reviews: "GET /api/google/contributor-reviews",
        /// Google local
        #[tool(catalog = "google.serp.local")]
        google_local: "GET /api/google/local",
        /// Google maps
        #[tool(catalog = "google.serp.maps")]
        google_maps: "GET /api/google/maps",
        /// Google maps photo meta
        #[tool(catalog = "google.maps.photo_metadata")]
        google_maps_photo_meta: "GET /api/google/maps/photo-meta",
        /// Google maps popular times
        #[tool(catalog = "google.maps.popular_times")]
        google_maps_popular_times: "GET /api/google/maps/popular-times",
        /// Google maps posts
        #[tool(catalog = "google.maps.posts")]
        google_maps_posts: "GET /api/google/maps/posts",
        /// Google play apps
        #[tool(catalog = "google-play.search.apps")]
        google_play_apps: "GET /api/google/play/apps",
        /// Google play books
        #[tool(catalog = "google-play.search.books")]
        google_play_books: "GET /api/google/play/books",
        /// Google play games
        #[tool(catalog = "google-play.search.games")]
        google_play_games: "GET /api/google/play/games",
        /// Google play movies
        #[tool(catalog = "google-play.search.movies")]
        google_play_movies: "GET /api/google/play/movies",
        /// Google play product
        #[tool(catalog = "google-play.product")]
        google_play_product: "GET /api/google/play/product",
        /// Google play reviews
        #[tool(catalog = "google-play.reviews")]
        google_play_reviews: "GET /api/google/play/reviews",
        /// Google reviews
        #[tool(catalog = "google.maps.reviews")]
        google_reviews: "GET /api/google/reviews",
        /// Google search
        #[tool(catalog = "google.serp.organic")]
        google_search: "GET /api/google/search",
        /// Google shopping
        #[tool(catalog = "google.serp.shopping")]
        google_shopping: "GET /api/google/shopping",
        /// Google shopping product
        #[tool(catalog = "google.shopping.product")]
        google_shopping_product: "GET /api/google/shopping/product",
        /// Tripadvisor place
        #[tool(catalog = "tripadvisor.place.detail")]
        tripadvisor_place: "GET /api/tripadvisor/place",
        /// Tripadvisor reviews
        #[tool(catalog = "tripadvisor.business.reviews")]
        tripadvisor_reviews: "GET /api/tripadvisor/reviews",
        /// Tripadvisor search
        #[tool(catalog = "tripadvisor.search.general")]
        tripadvisor_search: "GET /api/tripadvisor/search",
        /// Web fetch
        #[tool(catalog = "web.extract")]
        web_fetch: "GET /api/web/fetch",
        /// Web fetch post
        #[tool(catalog = "web.extract.advanced")]
        web_fetch_post: "POST /api/web/fetch",
        /// Web screenshot
        #[tool(catalog = "web.screenshot")]
        web_screenshot: "GET /api/web/screenshot",
        /// Web screenshot post
        #[tool(catalog = "web.screenshot.advanced")]
        web_screenshot_post: "POST /api/web/screenshot",
        /// Yelp reviews
        #[tool(catalog = "yelp.business.reviews")]
        yelp_reviews: "GET /api/yelp/reviews",
        /// Yelp search
        #[tool(catalog = "yelp.business.search")]
        yelp_search: "GET /api/yelp/search",
    },
}
