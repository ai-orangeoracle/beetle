//! GET /wifi：WiFi 配置页；GET /pairing：配对码设置页；GET /common.css：公共样式。均无需带码。
//! 构建时从 config_page 目录精简后写入 OUT_DIR，减少 Flash 占用。

/// 返回内嵌的 WiFi 配置页 HTML（通过 /common.css 加载样式）。
pub fn html() -> &'static str {
    include_str!(concat!(
        env!("OUT_DIR"),
        "/config_page_min/wifi_config_page.html"
    ))
}

/// 返回内嵌的配对码设置页 HTML。
pub fn pairing_html() -> &'static str {
    include_str!(concat!(
        env!("OUT_DIR"),
        "/config_page_min/pairing_page.html"
    ))
}

/// 返回公共 CSS 内容，供 GET /common.css 使用。
pub fn common_css() -> &'static str {
    include_str!(concat!(env!("OUT_DIR"), "/config_page_min/common.css"))
}

/// 返回公共 JS（菜单等），供 GET /common.js 使用。
pub fn common_js() -> &'static str {
    include_str!(concat!(env!("OUT_DIR"), "/config_page_min/common.js"))
}
