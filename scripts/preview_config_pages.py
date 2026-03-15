#!/usr/bin/env python3
# 本地预览配对页、WiFi 配置页、系统信息页样式（从项目根执行：python scripts/preview_config_pages.py）

import http.server
import os
import sys

# 项目根目录
ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
PAGES = os.path.join(ROOT, "src", "platform", "config_page")

FILES = {
    "/pairing": "pairing_page.html",
    "/wifi": "wifi_config_page.html",
    "/system": "system_info_page.html",
    "/common.css": "common.css",
    "/common.js": "common.js",
}

# 供页面 JS 请求的 mock 数据，便于看完整布局（含中文的用 str 再 encode）
MOCK_APIS = {
    "/": b'{"name":"beetle","version":"0.1.0-preview","endpoints":["GET /wifi","GET /system","POST /api/ota"]}',
    "/api/pairing_code": b'{"code_set":true}',
    "/api/config": b'{"wifi_ssid":"MyWiFi","wifi_pass":""}',
    "/api/system_info": '{"product_name":"beetle","system_status":"正常","current_time":"2025-03-09 12:00:00 UTC","firmware_version":"0.1.0-preview","ota_available":true}'.encode(
        "utf-8"
    ),
}


class Handler(http.server.BaseHTTPRequestHandler):
    def do_GET(self):
        path = self.path.split("?")[0].rstrip("/") or "/"
        if path in ("/", "/pairing", "/wifi", "/system"):
            if path == "/":
                path = "/wifi"  # 根重定向到 WiFi 配置
            if path in FILES:
                filepath = os.path.join(PAGES, FILES[path])
                if os.path.isfile(filepath):
                    self.send_response(200)
                    self.send_header("Content-Type", "text/html; charset=utf-8")
                    self.end_headers()
                    with open(filepath, "rb") as f:
                        self.wfile.write(f.read())
                    return
        if path in FILES:
            filepath = os.path.join(PAGES, FILES[path])
            if os.path.isfile(filepath):
                self.send_response(200)
                if path.endswith(".css"):
                    ct = "text/css; charset=utf-8"
                elif path.endswith(".js"):
                    ct = "application/javascript; charset=utf-8"
                else:
                    ct = "text/html; charset=utf-8"
                self.send_header("Content-Type", ct)
                self.end_headers()
                with open(filepath, "rb") as f:
                    self.wfile.write(f.read())
                return
        if path in MOCK_APIS:
            self.send_response(200)
            self.send_header("Content-Type", "application/json; charset=utf-8")
            self.send_header("Access-Control-Allow-Origin", "*")
            self.end_headers()
            self.wfile.write(MOCK_APIS[path])
            return
        self.send_response(404)
        self.end_headers()

    def log_message(self, format, *args):
        print(format % args)


def main():
    if not os.path.isdir(PAGES):
        print("Error: config_page dir not found:", PAGES, file=sys.stderr)
        sys.exit(1)
    port = 8765
    server = http.server.HTTPServer(("127.0.0.1", port), Handler)
    print("Config pages preview: http://127.0.0.1:%s" % port)
    print("  /pairing  配对码页")
    print("  /wifi     WiFi 配置页")
    print("  /system   系统信息页")
    print("Ctrl+C 退出")
    server.serve_forever()


if __name__ == "__main__":
    main()
