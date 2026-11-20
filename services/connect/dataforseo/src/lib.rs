#![no_std]
extern crate alloc;
use berm_lang::http::Client;
mod response;

const CLIENT: Client<'static> = Client {
    name: "DataForSEO",
    headers: &[("Authorization", "Basic {secret:DATAFORSEO_AUTH_TOKEN}")],
};

crab_openapi::service! {
    spec: env!("OPENAPI_SPEC"),
    client: crate::CLIENT,
    response: crate::response::check,
    tools: cfg(feature = "tools"),
    operations: {
        /// Backlinks summary
        #[tool(catalog = "web.backlinks.summary")]
        backlinks_summary: "POST /v3/backlinks/summary/live",
        /// Backlinks list
        #[tool(catalog = "web.backlinks.list")]
        backlinks_list: "POST /v3/backlinks/backlinks/live",
        /// Linking domains
        #[tool(catalog = "web.linking_domains.list")]
        linking_domains: "POST /v3/backlinks/referring_domains/live",
        /// Anchors
        #[tool(catalog = "web.anchors.list")]
        anchors: "POST /v3/backlinks/anchors/live",
        /// Url metrics
        #[tool(catalog = "web.url.metrics")]
        url_metrics: "POST /v3/backlinks/bulk_ranks/live",
        /// Backlink competitors
        #[tool(catalog = "web.backlinks.competitors")]
        backlink_competitors: "POST /v3/backlinks/competitors/live",
        /// Google serp
        #[tool(catalog = "google.serp.organic")]
        google_serp: "POST /v3/serp/google/organic/live/regular",
        /// Keyword volume
        #[tool(catalog = "google.keywords.volume")]
        keyword_volume: "POST /v3/keywords_data/google_ads/search_volume/live",
        /// Keyword ideas
        #[tool(catalog = "google.keywords.ideas")]
        keyword_ideas: "POST /v3/dataforseo_labs/google/related_keywords/live",
        /// Ranked keywords
        #[tool(catalog = "google.domain.ranked_keywords")]
        ranked_keywords: "POST /v3/dataforseo_labs/google/ranked_keywords/live",
        /// Page audit
        #[tool(catalog = "web.page.audit")]
        page_audit: "POST /v3/on_page/instant_pages",
        /// Account usage
        account_usage: "GET /v3/appendix/user_data",
        /// Ai optimization ai keyword data keywords search volume live
        ai_ai_keyword_data_keywords_search_volume_live: "POST /v3/ai_optimization/ai_keyword_data/keywords_search_volume/live",
        /// Ai optimization llm mentions historical live
        ai_llm_mentions_historical_live: "POST /v3/ai_optimization/llm_mentions/historical/live",
        /// Ai optimization llm mentions multi target metrics live
        ai_llm_mentions_multi_target_metrics_live: "POST /v3/ai_optimization/llm_mentions/multi_target_metrics/live",
        /// Ai optimization llm mentions search mentions live
        ai_llm_mentions_search_mentions_live: "POST /v3/ai_optimization/llm_mentions/search_mentions/live",
        /// Ai optimization llm mentions target metrics live
        ai_llm_mentions_target_metrics_live: "POST /v3/ai_optimization/llm_mentions/target_metrics/live",
        /// Ai optimization llm mentions target metrics lite live
        ai_llm_mentions_target_metrics_lite_live: "POST /v3/ai_optimization/llm_mentions/target_metrics_lite/live",
        /// Ai optimization llm mentions timeseries delta live
        ai_llm_mentions_timeseries_delta_live: "POST /v3/ai_optimization/llm_mentions/timeseries_delta/live",
        /// Ai optimization llm mentions timeseries new lost live
        ai_llm_mentions_timeseries_new_lost_live: "POST /v3/ai_optimization/llm_mentions/timeseries_new_lost/live",
        /// Top brand categories mentioned by LLMs
        ai_llm_mentions_top_mentioned_brand_categories_live: "POST /v3/ai_optimization/llm_mentions/top_mentioned_brand_categories/live",
        /// Top brand categories mentioned by LLMs (Lite)
        ai_llm_mentions_top_mentioned_brand_categories_lite_live: "POST /v3/ai_optimization/llm_mentions/top_mentioned_brand_categories_lite/live",
        /// Ai optimization llm mentions top mentioned brands live
        ai_llm_mentions_top_mentioned_brands_live: "POST /v3/ai_optimization/llm_mentions/top_mentioned_brands/live",
        /// Ai optimization llm mentions top mentioned brands lite live
        ai_llm_mentions_top_mentioned_brands_lite_live: "POST /v3/ai_optimization/llm_mentions/top_mentioned_brands_lite/live",
        /// Ai optimization llm mentions top mentioned domains live
        ai_llm_mentions_top_mentioned_domains_live: "POST /v3/ai_optimization/llm_mentions/top_mentioned_domains/live",
        /// Ai optimization llm mentions top mentioned domains lite live
        ai_llm_mentions_top_mentioned_domains_lite_live: "POST /v3/ai_optimization/llm_mentions/top_mentioned_domains_lite/live",
        /// Ai optimization llm mentions top mentioned pages live
        ai_llm_mentions_top_mentioned_pages_live: "POST /v3/ai_optimization/llm_mentions/top_mentioned_pages/live",
        /// Ai optimization llm mentions top mentioned pages lite live
        ai_llm_mentions_top_mentioned_pages_lite_live: "POST /v3/ai_optimization/llm_mentions/top_mentioned_pages_lite/live",
        /// Dataforseo labs amazon bulk search volume live
        labs_amazon_bulk_search_volume_live: "POST /v3/dataforseo_labs/amazon/bulk_search_volume/live",
        /// Dataforseo labs amazon product competitors live
        labs_amazon_product_competitors_live: "POST /v3/dataforseo_labs/amazon/product_competitors/live",
        /// Dataforseo labs amazon product keyword intersections live
        labs_amazon_product_keyword_intersections_live: "POST /v3/dataforseo_labs/amazon/product_keyword_intersections/live",
        /// Dataforseo labs amazon product rank overview live
        labs_amazon_product_rank_overview_live: "POST /v3/dataforseo_labs/amazon/product_rank_overview/live",
        /// Dataforseo labs amazon ranked keywords live
        labs_amazon_ranked_keywords_live: "POST /v3/dataforseo_labs/amazon/ranked_keywords/live",
        /// Dataforseo labs amazon related keywords live
        labs_amazon_related_keywords_live: "POST /v3/dataforseo_labs/amazon/related_keywords/live",
        /// Merchant amazon asin live advanced
        merchant_amazon_asin_live_advanced: "POST /v3/merchant/amazon/asin/live/advanced",
        /// Merchant amazon locations country
        merchant_amazon_locations_country: "GET /v3/merchant/amazon/locations/{country}",
        /// Merchant amazon products live advanced
        merchant_amazon_products_live_advanced: "POST /v3/merchant/amazon/products/live/advanced",
        /// Merchant amazon sellers live advanced
        merchant_amazon_sellers_live_advanced: "POST /v3/merchant/amazon/sellers/live/advanced",
        /// App data apple app info task get advanced id
        app_data_apple_app_info_task_get_advanced_id: "GET /v3/app_data/apple/app_info/task_get/advanced/{id}",
        /// App data apple app info task post
        app_data_apple_app_info_task_post: "POST /v3/app_data/apple/app_info/task_post",
        /// App data apple app list task get advanced id
        app_data_apple_app_list_task_get_advanced_id: "GET /v3/app_data/apple/app_list/task_get/advanced/{id}",
        /// App data apple app list task post
        app_data_apple_app_list_task_post: "POST /v3/app_data/apple/app_list/task_post",
        /// App data apple app listings search live
        app_data_apple_app_listings_search_live: "POST /v3/app_data/apple/app_listings/search/live",
        /// App data apple app reviews task get advanced id
        app_data_apple_app_reviews_task_get_advanced_id: "GET /v3/app_data/apple/app_reviews/task_get/advanced/{id}",
        /// App data apple app reviews task post
        app_data_apple_app_reviews_task_post: "POST /v3/app_data/apple/app_reviews/task_post",
        /// App data apple app searches task get advanced id
        app_data_apple_app_searches_task_get_advanced_id: "GET /v3/app_data/apple/app_searches/task_get/advanced/{id}",
        /// App data apple app searches task post
        app_data_apple_app_searches_task_post: "POST /v3/app_data/apple/app_searches/task_post",
        /// Dataforseo labs apple app competitors live
        labs_apple_app_competitors_live: "POST /v3/dataforseo_labs/apple/app_competitors/live",
        /// Dataforseo labs apple app intersection live
        labs_apple_app_intersection_live: "POST /v3/dataforseo_labs/apple/app_intersection/live",
        /// Dataforseo labs apple bulk app metrics live
        labs_apple_bulk_app_metrics_live: "POST /v3/dataforseo_labs/apple/bulk_app_metrics/live",
        /// Dataforseo labs apple keywords for app live
        labs_apple_keywords_for_app_live: "POST /v3/dataforseo_labs/apple/keywords_for_app/live",
        /// Serp baidu locations country
        serp_baidu_locations_country: "GET /v3/serp/baidu/locations/{country}",
        /// Serp baidu organic task get advanced id
        serp_baidu_organic_task_get_advanced_id: "GET /v3/serp/baidu/organic/task_get/advanced/{id}",
        /// Serp baidu organic task post
        serp_baidu_organic_task_post: "POST /v3/serp/baidu/organic/task_post",
        /// Keywords data bing audience estimation industries
        keywords_data_bing_audience_estimation_industries: "GET /v3/keywords_data/bing/audience_estimation/industries",
        /// Keywords data bing audience estimation job functions
        keywords_data_bing_audience_estimation_job_functions: "GET /v3/keywords_data/bing/audience_estimation/job_functions",
        /// Keywords data bing audience estimation live
        keywords_data_bing_audience_estimation_live: "POST /v3/keywords_data/bing/audience_estimation/live",
        /// Keywords data bing keyword performance live
        keywords_data_bing_keyword_performance_live: "POST /v3/keywords_data/bing/keyword_performance/live",
        /// Keywords data bing keywords for keywords live
        keywords_data_bing_keywords_for_keywords_live: "POST /v3/keywords_data/bing/keywords_for_keywords/live",
        /// Keywords data bing keywords for site live
        keywords_data_bing_keywords_for_site_live: "POST /v3/keywords_data/bing/keywords_for_site/live",
        /// Keywords data bing search volume live
        keywords_data_bing_search_volume_live: "POST /v3/keywords_data/bing/search_volume/live",
        /// Keywords data bing search volume history live
        keywords_data_bing_search_volume_history_live: "POST /v3/keywords_data/bing/search_volume_history/live",
        /// Serp bing locations country
        serp_bing_locations_country: "GET /v3/serp/bing/locations/{country}",
        /// Serp bing organic live advanced
        #[tool(catalog = "bing.serp.organic")]
        serp_bing_organic_live_advanced: "POST /v3/serp/bing/organic/live/advanced",
        /// Serp bing organic live regular
        #[tool(catalog = "bing.serp.organic")]
        serp_bing_organic_live_regular: "POST /v3/serp/bing/organic/live/regular",
        /// Ai optimization chat gpt llm responses live
        ai_chat_gpt_llm_responses_live: "POST /v3/ai_optimization/chat_gpt/llm_responses/live",
        /// Ai optimization chat gpt llm scraper live advanced
        ai_chat_gpt_llm_scraper_live_advanced: "POST /v3/ai_optimization/chat_gpt/llm_scraper/live/advanced",
        /// Ai optimization chat gpt llm scraper locations country
        ai_chat_gpt_llm_scraper_locations_country: "GET /v3/ai_optimization/chat_gpt/llm_scraper/locations/{country}",
        /// Ai optimization claude llm responses live
        ai_claude_llm_responses_live: "POST /v3/ai_optimization/claude/llm_responses/live",
        /// Ai optimization gemini llm responses live
        ai_gemini_llm_responses_live: "POST /v3/ai_optimization/gemini/llm_responses/live",
        /// Ai optimization gemini llm scraper live advanced
        ai_gemini_llm_scraper_live_advanced: "POST /v3/ai_optimization/gemini/llm_scraper/live/advanced",
        /// Business data google extended reviews task get id
        business_data_google_extended_reviews_task_get_id: "GET /v3/business_data/google/extended_reviews/task_get/{id}",
        /// Business data google extended reviews task post
        business_data_google_extended_reviews_task_post: "POST /v3/business_data/google/extended_reviews/task_post",
        /// Business data google hotel info live advanced
        business_data_google_hotel_info_live_advanced: "POST /v3/business_data/google/hotel_info/live/advanced",
        /// Business data google hotel searches live
        business_data_google_hotel_searches_live: "POST /v3/business_data/google/hotel_searches/live",
        /// Business data google locations country
        business_data_google_locations_country: "GET /v3/business_data/google/locations/{country}",
        /// Business data google my business info live
        business_data_google_my_business_info_live: "POST /v3/business_data/google/my_business_info/live",
        /// Business data google my business updates task get id
        business_data_google_my_business_updates_task_get_id: "GET /v3/business_data/google/my_business_updates/task_get/{id}",
        /// Business data google my business updates task post
        business_data_google_my_business_updates_task_post: "POST /v3/business_data/google/my_business_updates/task_post",
        /// Business data google questions and answers live
        business_data_google_questions_and_answers_live: "POST /v3/business_data/google/questions_and_answers/live",
        /// Business data google reviews task get id
        business_data_google_reviews_task_get_id: "GET /v3/business_data/google/reviews/task_get/{id}",
        /// Business data google reviews task post
        business_data_google_reviews_task_post: "POST /v3/business_data/google/reviews/task_post",
        /// Dataforseo labs google app competitors live
        labs_google_app_competitors_live: "POST /v3/dataforseo_labs/google/app_competitors/live",
        /// Dataforseo labs google app intersection live
        labs_google_app_intersection_live: "POST /v3/dataforseo_labs/google/app_intersection/live",
        /// Dataforseo labs google available history
        labs_google_available_history: "GET /v3/dataforseo_labs/google/available_history",
        /// Dataforseo labs google bulk app metrics live
        labs_google_bulk_app_metrics_live: "POST /v3/dataforseo_labs/google/bulk_app_metrics/live",
        /// Dataforseo labs google bulk keyword difficulty live
        labs_google_bulk_keyword_difficulty_live: "POST /v3/dataforseo_labs/google/bulk_keyword_difficulty/live",
        /// Dataforseo labs google bulk traffic estimation live
        labs_google_bulk_traffic_estimation_live: "POST /v3/dataforseo_labs/google/bulk_traffic_estimation/live",
        /// Dataforseo labs google categories for domain live
        labs_google_categories_for_domain_live: "POST /v3/dataforseo_labs/google/categories_for_domain/live",
        /// Dataforseo labs google categories for keywords live
        labs_google_categories_for_keywords_live: "POST /v3/dataforseo_labs/google/categories_for_keywords/live",
        /// Dataforseo labs google competitors domain live
        labs_google_competitors_domain_live: "POST /v3/dataforseo_labs/google/competitors_domain/live",
        /// Dataforseo labs google domain intersection live
        labs_google_domain_intersection_live: "POST /v3/dataforseo_labs/google/domain_intersection/live",
        /// Dataforseo labs google domain metrics by categories live
        labs_google_domain_metrics_by_categories_live: "POST /v3/dataforseo_labs/google/domain_metrics_by_categories/live",
        /// Dataforseo labs google domain rank overview live
        labs_google_domain_rank_overview_live: "POST /v3/dataforseo_labs/google/domain_rank_overview/live",
        /// Estimate historical Google traffic for multiple targets
        labs_google_historical_bulk_traffic_estimation_live: "POST /v3/dataforseo_labs/google/historical_bulk_traffic_estimation/live",
        /// Dataforseo labs google historical keyword data live
        labs_google_historical_keyword_data_live: "POST /v3/dataforseo_labs/google/historical_keyword_data/live",
        /// Dataforseo labs google historical rank overview live
        labs_google_historical_rank_overview_live: "POST /v3/dataforseo_labs/google/historical_rank_overview/live",
        /// Dataforseo labs google historical serps live
        labs_google_historical_serps_live: "POST /v3/dataforseo_labs/google/historical_serps/live",
        /// Dataforseo labs google keyword ideas live
        labs_google_keyword_ideas_live: "POST /v3/dataforseo_labs/google/keyword_ideas/live",
        /// Dataforseo labs google keyword overview live
        labs_google_keyword_overview_live: "POST /v3/dataforseo_labs/google/keyword_overview/live",
        /// Dataforseo labs google keyword suggestions live
        labs_google_keyword_suggestions_live: "POST /v3/dataforseo_labs/google/keyword_suggestions/live",
        /// Dataforseo labs google keywords for app live
        labs_google_keywords_for_app_live: "POST /v3/dataforseo_labs/google/keywords_for_app/live",
        /// Dataforseo labs google keywords for categories live
        labs_google_keywords_for_categories_live: "POST /v3/dataforseo_labs/google/keywords_for_categories/live",
        /// Dataforseo labs google keywords for site live
        labs_google_keywords_for_site_live: "POST /v3/dataforseo_labs/google/keywords_for_site/live",
        /// Dataforseo labs google page intersection live
        labs_google_page_intersection_live: "POST /v3/dataforseo_labs/google/page_intersection/live",
        /// Dataforseo labs google relevant pages live
        labs_google_relevant_pages_live: "POST /v3/dataforseo_labs/google/relevant_pages/live",
        /// Dataforseo labs google search intent live
        labs_google_search_intent_live: "POST /v3/dataforseo_labs/google/search_intent/live",
        /// Dataforseo labs google serp competitors live
        labs_google_serp_competitors_live: "POST /v3/dataforseo_labs/google/serp_competitors/live",
        /// Dataforseo labs google subdomains live
        labs_google_subdomains_live: "POST /v3/dataforseo_labs/google/subdomains/live",
        /// Dataforseo labs google top searches live
        labs_google_top_searches_live: "POST /v3/dataforseo_labs/google/top_searches/live",
        /// Keywords data google ads ad traffic by keywords live
        keywords_data_google_ads_ad_traffic_by_keywords_live: "POST /v3/keywords_data/google_ads/ad_traffic_by_keywords/live",
        /// Keywords data google ads keywords for keywords live
        keywords_data_google_ads_keywords_for_keywords_live: "POST /v3/keywords_data/google_ads/keywords_for_keywords/live",
        /// Keywords data google ads keywords for site live
        keywords_data_google_ads_keywords_for_site_live: "POST /v3/keywords_data/google_ads/keywords_for_site/live",
        /// Keywords data google ads locations country
        keywords_data_google_ads_locations_country: "GET /v3/keywords_data/google_ads/locations/{country}",
        /// Keywords data google trends explore live
        keywords_data_google_trends_explore_live: "POST /v3/keywords_data/google_trends/explore/live",
        /// Keywords data google trends locations country
        keywords_data_google_trends_locations_country: "GET /v3/keywords_data/google_trends/locations/{country}",
        /// Merchant google locations country
        merchant_google_locations_country: "GET /v3/merchant/google/locations/{country}",
        /// Merchant google product info task get advanced id
        merchant_google_product_info_task_get_advanced_id: "GET /v3/merchant/google/product_info/task_get/advanced/{id}",
        /// Merchant google product info task post
        merchant_google_product_info_task_post: "POST /v3/merchant/google/product_info/task_post",
        /// Merchant google products task get advanced id
        merchant_google_products_task_get_advanced_id: "GET /v3/merchant/google/products/task_get/advanced/{id}",
        /// Merchant google products task post
        merchant_google_products_task_post: "POST /v3/merchant/google/products/task_post",
        /// Merchant google sellers ad url shop ad aclk
        merchant_google_sellers_ad_url_shop_ad_aclk: "GET /v3/merchant/google/sellers/ad_url/{shop_ad_aclk}",
        /// Merchant google sellers task get advanced id
        merchant_google_sellers_task_get_advanced_id: "GET /v3/merchant/google/sellers/task_get/advanced/{id}",
        /// Merchant google sellers task post
        merchant_google_sellers_task_post: "POST /v3/merchant/google/sellers/task_post",
        /// Serp ai summary
        serp_ai_summary: "POST /v3/serp/ai_summary",
        /// Serp google ads advertisers task get advanced id
        serp_google_ads_advertisers_task_get_advanced_id: "GET /v3/serp/google/ads_advertisers/task_get/advanced/{id}",
        /// Serp google ads advertisers task post
        serp_google_ads_advertisers_task_post: "POST /v3/serp/google/ads_advertisers/task_post",
        /// Serp google ads search task get advanced id
        serp_google_ads_search_task_get_advanced_id: "GET /v3/serp/google/ads_search/task_get/advanced/{id}",
        /// Serp google ads search task post
        serp_google_ads_search_task_post: "POST /v3/serp/google/ads_search/task_post",
        /// Serp google ai mode live advanced
        serp_google_ai_mode_live_advanced: "POST /v3/serp/google/ai_mode/live/advanced",
        /// Serp google autocomplete live advanced
        serp_google_autocomplete_live_advanced: "POST /v3/serp/google/autocomplete/live/advanced",
        /// Serp google dataset info live advanced
        serp_google_dataset_info_live_advanced: "POST /v3/serp/google/dataset_info/live/advanced",
        /// Serp google dataset search live advanced
        serp_google_dataset_search_live_advanced: "POST /v3/serp/google/dataset_search/live/advanced",
        /// Serp google finance explore live advanced
        serp_google_finance_explore_live_advanced: "POST /v3/serp/google/finance_explore/live/advanced",
        /// Serp google finance markets live advanced
        serp_google_finance_markets_live_advanced: "POST /v3/serp/google/finance_markets/live/advanced",
        /// Serp google finance quote live advanced
        serp_google_finance_quote_live_advanced: "POST /v3/serp/google/finance_quote/live/advanced",
        /// Serp google finance ticker search live advanced
        serp_google_finance_ticker_search_live_advanced: "POST /v3/serp/google/finance_ticker_search/live/advanced",
        /// Serp google images live advanced
        #[tool(catalog = "google.serp.images")]
        serp_google_images_live_advanced: "POST /v3/serp/google/images/live/advanced",
        /// Serp google jobs task get advanced id
        serp_google_jobs_task_get_advanced_id: "GET /v3/serp/google/jobs/task_get/advanced/{id}",
        /// Serp google jobs task post
        serp_google_jobs_task_post: "POST /v3/serp/google/jobs/task_post",
        /// Serp google local finder live advanced
        serp_google_local_finder_live_advanced: "POST /v3/serp/google/local_finder/live/advanced",
        /// Serp google locations country
        serp_google_locations_country: "GET /v3/serp/google/locations/{country}",
        /// Serp google maps live advanced
        #[tool(catalog = "google.serp.maps")]
        serp_google_maps_live_advanced: "POST /v3/serp/google/maps/live/advanced",
        /// Serp google news live advanced
        #[tool(catalog = "google.serp.news")]
        serp_google_news_live_advanced: "POST /v3/serp/google/news/live/advanced",
        /// Serp google organic live advanced
        #[tool(catalog = "google.serp.organic")]
        serp_google_organic_live_advanced: "POST /v3/serp/google/organic/live/advanced",
        /// Serp google search by image task get advanced id
        serp_google_search_by_image_task_get_advanced_id: "GET /v3/serp/google/search_by_image/task_get/advanced/{id}",
        /// Serp google search by image task post
        serp_google_search_by_image_task_post: "POST /v3/serp/google/search_by_image/task_post",
        /// Serp screenshot
        serp_screenshot: "POST /v3/serp/screenshot",
        /// App data google app info task get advanced id
        app_data_google_app_info_task_get_advanced_id: "GET /v3/app_data/google/app_info/task_get/advanced/{id}",
        /// App data google app info task post
        app_data_google_app_info_task_post: "POST /v3/app_data/google/app_info/task_post",
        /// App data google app list task get advanced id
        app_data_google_app_list_task_get_advanced_id: "GET /v3/app_data/google/app_list/task_get/advanced/{id}",
        /// App data google app list task post
        app_data_google_app_list_task_post: "POST /v3/app_data/google/app_list/task_post",
        /// App data google app listings search live
        app_data_google_app_listings_search_live: "POST /v3/app_data/google/app_listings/search/live",
        /// App data google app reviews task get advanced id
        app_data_google_app_reviews_task_get_advanced_id: "GET /v3/app_data/google/app_reviews/task_get/advanced/{id}",
        /// App data google app reviews task post
        app_data_google_app_reviews_task_post: "POST /v3/app_data/google/app_reviews/task_post",
        /// App data google app searches task get advanced id
        app_data_google_app_searches_task_get_advanced_id: "GET /v3/app_data/google/app_searches/task_get/advanced/{id}",
        /// App data google app searches task post
        app_data_google_app_searches_task_post: "POST /v3/app_data/google/app_searches/task_post",
        /// App data google locations country
        app_data_google_locations_country: "GET /v3/app_data/google/locations/{country}",
        /// Serp naver organic task get advanced id
        serp_naver_organic_task_get_advanced_id: "GET /v3/serp/naver/organic/task_get/advanced/{id}",
        /// Serp naver organic task post
        serp_naver_organic_task_post: "POST /v3/serp/naver/organic/task_post",
        /// Ai optimization perplexity llm responses live
        ai_perplexity_llm_responses_live: "POST /v3/ai_optimization/perplexity/llm_responses/live",
        /// Serp seznam locations country
        serp_seznam_locations_country: "GET /v3/serp/seznam/locations/{country}",
        /// Serp seznam organic task get advanced id
        serp_seznam_organic_task_get_advanced_id: "GET /v3/serp/seznam/organic/task_get/advanced/{id}",
        /// Serp seznam organic task post
        serp_seznam_organic_task_post: "POST /v3/serp/seznam/organic/task_post",
        /// Business data tripadvisor locations country
        business_data_tripadvisor_locations_country: "GET /v3/business_data/tripadvisor/locations/{country}",
        /// Business data tripadvisor reviews task get id
        business_data_tripadvisor_reviews_task_get_id: "GET /v3/business_data/tripadvisor/reviews/task_get/{id}",
        /// Business data tripadvisor reviews task post
        business_data_tripadvisor_reviews_task_post: "POST /v3/business_data/tripadvisor/reviews/task_post",
        /// Business data tripadvisor search task get id
        business_data_tripadvisor_search_task_get_id: "GET /v3/business_data/tripadvisor/search/task_get/{id}",
        /// Business data tripadvisor search task post
        business_data_tripadvisor_search_task_post: "POST /v3/business_data/tripadvisor/search/task_post",
        /// Business data trustpilot reviews task get id
        business_data_trustpilot_reviews_task_get_id: "GET /v3/business_data/trustpilot/reviews/task_get/{id}",
        /// Business data trustpilot reviews task post
        business_data_trustpilot_reviews_task_post: "POST /v3/business_data/trustpilot/reviews/task_post",
        /// Business data trustpilot search task get id
        business_data_trustpilot_search_task_get_id: "GET /v3/business_data/trustpilot/search/task_get/{id}",
        /// Business data trustpilot search task post
        business_data_trustpilot_search_task_post: "POST /v3/business_data/trustpilot/search/task_post",
        /// Backlinks bulk backlinks live
        backlinks_bulk_backlinks_live: "POST /v3/backlinks/bulk_backlinks/live",
        /// Backlinks bulk new lost backlinks live
        backlinks_bulk_new_lost_backlinks_live: "POST /v3/backlinks/bulk_new_lost_backlinks/live",
        /// Backlinks bulk new lost referring domains live
        backlinks_bulk_new_lost_referring_domains_live: "POST /v3/backlinks/bulk_new_lost_referring_domains/live",
        /// Backlinks bulk pages summary live
        backlinks_bulk_pages_summary_live: "POST /v3/backlinks/bulk_pages_summary/live",
        /// Backlinks bulk referring domains live
        backlinks_bulk_referring_domains_live: "POST /v3/backlinks/bulk_referring_domains/live",
        /// Backlinks bulk spam score live
        backlinks_bulk_spam_score_live: "POST /v3/backlinks/bulk_spam_score/live",
        /// Backlinks domain intersection live
        backlinks_domain_intersection_live: "POST /v3/backlinks/domain_intersection/live",
        /// Backlinks domain pages live
        backlinks_domain_pages_live: "POST /v3/backlinks/domain_pages/live",
        /// Backlinks domain pages summary live
        backlinks_domain_pages_summary_live: "POST /v3/backlinks/domain_pages_summary/live",
        /// Backlinks history live
        backlinks_history_live: "POST /v3/backlinks/history/live",
        /// Backlinks index
        backlinks_index: "GET /v3/backlinks/index",
        /// Backlinks page intersection live
        backlinks_page_intersection_live: "POST /v3/backlinks/page_intersection/live",
        /// Backlinks referring networks live
        backlinks_referring_networks_live: "POST /v3/backlinks/referring_networks/live",
        /// Backlinks timeseries new lost summary live
        backlinks_timeseries_new_lost_summary_live: "POST /v3/backlinks/timeseries_new_lost_summary/live",
        /// Backlinks timeseries summary live
        backlinks_timeseries_summary_live: "POST /v3/backlinks/timeseries_summary/live",
        /// Business data business listings categories aggregation live
        business_data_business_listings_categories_aggregation_live: "POST /v3/business_data/business_listings/categories_aggregation/live",
        /// Business data business listings search live
        business_data_business_listings_search_live: "POST /v3/business_data/business_listings/search/live",
        /// Content analysis category trends live
        content_analysis_category_trends_live: "POST /v3/content_analysis/category_trends/live",
        /// Content analysis phrase trends live
        content_analysis_phrase_trends_live: "POST /v3/content_analysis/phrase_trends/live",
        /// Content analysis rating distribution live
        content_analysis_rating_distribution_live: "POST /v3/content_analysis/rating_distribution/live",
        /// Content analysis search live
        content_analysis_search_live: "POST /v3/content_analysis/search/live",
        /// Content analysis sentiment analysis live
        content_analysis_sentiment_analysis_live: "POST /v3/content_analysis/sentiment_analysis/live",
        /// Content analysis summary live
        content_analysis_summary_live: "POST /v3/content_analysis/summary/live",
        /// Domain analytics technologies aggregation technologies live
        domain_analytics_technologies_aggregation_technologies_live: "POST /v3/domain_analytics/technologies/aggregation_technologies/live",
        /// Domain analytics technologies domain technologies live
        domain_analytics_technologies_domain_technologies_live: "POST /v3/domain_analytics/technologies/domain_technologies/live",
        /// Domain analytics technologies domains by html terms live
        domain_analytics_technologies_domains_by_html_terms_live: "POST /v3/domain_analytics/technologies/domains_by_html_terms/live",
        /// Domain analytics technologies domains by technology live
        domain_analytics_technologies_domains_by_technology_live: "POST /v3/domain_analytics/technologies/domains_by_technology/live",
        /// Domain analytics technologies technologies
        domain_analytics_technologies_technologies: "GET /v3/domain_analytics/technologies/technologies",
        /// Domain analytics technologies technologies summary live
        domain_analytics_technologies_technologies_summary_live: "POST /v3/domain_analytics/technologies/technologies_summary/live",
        /// Domain analytics technologies technology stats live
        domain_analytics_technologies_technology_stats_live: "POST /v3/domain_analytics/technologies/technology_stats/live",
        /// Domain analytics whois overview live
        domain_analytics_whois_overview_live: "POST /v3/domain_analytics/whois/overview/live",
        /// Keywords data clickstream data bulk search volume live
        keywords_data_clickstream_data_bulk_search_volume_live: "POST /v3/keywords_data/clickstream_data/bulk_search_volume/live",
        /// Keywords data clickstream data dataforseo search volume live
        keywords_data_clickstream_data_dataforseo_search_volume_live: "POST /v3/keywords_data/clickstream_data/dataforseo_search_volume/live",
        /// Keywords data clickstream data global search volume live
        keywords_data_clickstream_data_global_search_volume_live: "POST /v3/keywords_data/clickstream_data/global_search_volume/live",
        /// Keywords data dataforseo trends demography live
        keywords_data_dataforseo_trends_demography_live: "POST /v3/keywords_data/dataforseo_trends/demography/live",
        /// Keywords data dataforseo trends explore live
        keywords_data_dataforseo_trends_explore_live: "POST /v3/keywords_data/dataforseo_trends/explore/live",
        /// Keywords data dataforseo trends locations country
        keywords_data_dataforseo_trends_locations_country: "GET /v3/keywords_data/dataforseo_trends/locations/{country}",
        /// Keywords data dataforseo trends merged data live
        keywords_data_dataforseo_trends_merged_data_live: "POST /v3/keywords_data/dataforseo_trends/merged_data/live",
        /// Keywords data dataforseo trends subregion interests live
        keywords_data_dataforseo_trends_subregion_interests_live: "POST /v3/keywords_data/dataforseo_trends/subregion_interests/live",
        /// On page content parsing
        on_page_content_parsing: "POST /v3/on_page/content_parsing",
        /// On page content parsing live
        on_page_content_parsing_live: "POST /v3/on_page/content_parsing/live",
        /// On page duplicate content
        on_page_duplicate_content: "POST /v3/on_page/duplicate_content",
        /// On page duplicate tags
        on_page_duplicate_tags: "POST /v3/on_page/duplicate_tags",
        /// On page keyword density
        on_page_keyword_density: "POST /v3/on_page/keyword_density",
        /// On page lighthouse audits
        on_page_lighthouse_audits: "GET /v3/on_page/lighthouse/audits",
        /// On page lighthouse live json
        on_page_lighthouse_live_json: "POST /v3/on_page/lighthouse/live/json",
        /// On page links
        on_page_links: "POST /v3/on_page/links",
        /// On page microdata
        on_page_microdata: "POST /v3/on_page/microdata",
        /// On page non indexable
        on_page_non_indexable: "POST /v3/on_page/non_indexable",
        /// On page page screenshot
        on_page_page_screenshot: "POST /v3/on_page/page_screenshot",
        /// On page pages
        on_page_pages: "POST /v3/on_page/pages",
        /// On page pages by resource
        on_page_pages_by_resource: "POST /v3/on_page/pages_by_resource",
        /// On page raw html
        on_page_raw_html: "POST /v3/on_page/raw_html",
        /// On page redirect chains
        on_page_redirect_chains: "POST /v3/on_page/redirect_chains",
        /// On page resources
        on_page_resources: "POST /v3/on_page/resources",
        /// On page summary id
        on_page_summary_id: "GET /v3/on_page/summary/{id}",
        /// On page task post
        on_page_task_post: "POST /v3/on_page/task_post",
        /// On page uncrawlable resources
        on_page_uncrawlable_resources: "POST /v3/on_page/uncrawlable_resources",
        /// On page waterfall
        on_page_waterfall: "POST /v3/on_page/waterfall",
        /// Serp yahoo locations country
        serp_yahoo_locations_country: "GET /v3/serp/yahoo/locations/{country}",
        /// Serp yahoo organic live advanced
        #[tool(catalog = "yahoo.serp.organic")]
        serp_yahoo_organic_live_advanced: "POST /v3/serp/yahoo/organic/live/advanced",
        /// Serp yahoo organic live regular
        #[tool(catalog = "yahoo.serp.organic")]
        serp_yahoo_organic_live_regular: "POST /v3/serp/yahoo/organic/live/regular",
        /// Serp youtube locations country
        serp_youtube_locations_country: "GET /v3/serp/youtube/locations/{country}",
        /// Serp youtube organic live advanced
        #[tool(catalog = "youtube.serp.organic")]
        serp_youtube_organic_live_advanced: "POST /v3/serp/youtube/organic/live/advanced",
        /// Serp youtube video comments live advanced
        serp_youtube_video_comments_live_advanced: "POST /v3/serp/youtube/video_comments/live/advanced",
        /// Serp youtube video info live advanced
        serp_youtube_video_info_live_advanced: "POST /v3/serp/youtube/video_info/live/advanced",
        /// Serp youtube video subtitles live advanced
        serp_youtube_video_subtitles_live_advanced: "POST /v3/serp/youtube/video_subtitles/live/advanced",
    },
}
