//! Host/Linux：无 TLS HTTP 栈时的桩；所有请求返回不支持错误。
//! Host stub when no embedded HTTP stack is linked.

use crate::config::AppConfig;
use crate::error::{Error, Result};
use crate::orchestrator::Priority;
use crate::platform::ResponseBody;

/// 桩：不发起真实 HTTP；与 `Platform::create_http_client` 在 Linux 上的行为一致。
pub struct EspHttpClient {
    _priv: (),
}

impl EspHttpClient {
    pub fn new() -> Result<Self> {
        Err(Error::config(
            "http_client_new",
            "EspHttpClient not available on this target",
        ))
    }

    pub fn new_with_priority(_priority: Priority) -> Result<Self> {
        Self::new()
    }

    pub fn new_with_config(_config: &AppConfig) -> Result<Self> {
        Self::new()
    }

    pub fn replace_connection(&mut self) -> Result<()> {
        Err(Error::config("http_client_replace", "host stub"))
    }

    pub fn get_with_headers_inner(
        &mut self,
        _url: &str,
        _headers: &[(&str, &str)],
    ) -> Result<(u16, ResponseBody)> {
        Err(Error::config("http_get_request", "host http stub"))
    }

    pub fn post_with_headers(
        &mut self,
        _url: &str,
        _headers: &[(&str, &str)],
        _body: &[u8],
    ) -> Result<(u16, ResponseBody)> {
        Err(Error::config("http_post_request", "host http stub"))
    }

    pub fn patch_with_headers(
        &mut self,
        _url: &str,
        _headers: &[(&str, &str)],
        _body: &[u8],
    ) -> Result<(u16, ResponseBody)> {
        Err(Error::config("http_patch_request", "host http stub"))
    }

    pub fn do_post_streaming(
        &mut self,
        _url: &str,
        _headers: &[(&str, &str)],
        _body: &[u8],
        _on_chunk: &mut dyn FnMut(&[u8]) -> Result<()>,
    ) -> Result<u16> {
        Err(Error::config("http_post_request", "host http stub"))
    }
}

impl crate::platform::PlatformHttpClient for EspHttpClient {
    fn get(&mut self, url: &str, headers: &[(&str, &str)]) -> Result<(u16, ResponseBody)> {
        self.get_with_headers_inner(url, headers)
    }
    fn post(
        &mut self,
        _url: &str,
        _headers: &[(&str, &str)],
        _body: &[u8],
    ) -> Result<(u16, ResponseBody)> {
        Err(Error::config("http_post_request", "host http stub"))
    }
    fn post_streaming(
        &mut self,
        url: &str,
        headers: &[(&str, &str)],
        body: &[u8],
        on_chunk: &mut dyn FnMut(&[u8]) -> Result<()>,
    ) -> Result<u16> {
        EspHttpClient::do_post_streaming(self, url, headers, body, on_chunk)
    }
    fn patch(
        &mut self,
        url: &str,
        headers: &[(&str, &str)],
        body: &[u8],
    ) -> Result<(u16, ResponseBody)> {
        self.patch_with_headers(url, headers, body)
    }
    fn put(
        &mut self,
        _url: &str,
        _headers: &[(&str, &str)],
        _body: &[u8],
    ) -> Result<(u16, ResponseBody)> {
        Err(Error::config("http_put_request", "host http stub"))
    }
    fn delete(&mut self, _url: &str, _headers: &[(&str, &str)]) -> Result<(u16, ResponseBody)> {
        Err(Error::config("http_delete_request", "host http stub"))
    }
    fn reset_connection_for_retry(&mut self) {
        let _ = self.replace_connection();
    }
}
