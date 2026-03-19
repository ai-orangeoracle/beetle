# Beetle Configure UI

[中文](README.zh-CN.md)

## What is this?

**Beetle Configure UI** is the config web app for **Beetle** (甲虫) firmware. It is a standalone frontend that talks to the device over HTTP: you open it in a browser, connect to the device (via its hotspot or same LAN), then set WiFi, LLM, channels, and other options. The UI is **not** shipped inside the firmware; it is either served by the device (built-in copy) or loaded from the online build (GitHub Pages), so the firmware stays small and the config experience can be updated without flashing.

## Why does this repo exist?

- **Keep firmware lean**: No large config UI bundle in flash; device can serve a minimal copy or you use the online version.
- **Single codebase**: One React app for both “open from device” and “open from web”; same features and i18n (zh-CN / en-US).
- **Easier updates**: Config UI can be improved and redeployed (e.g. GitHub Pages) without rebuilding or OTA’ing the firmware.

## What is it for?

After you connect to a Beetle device, this UI lets you:

- Set or change the **pairing code** (required for saving config).
- Configure **WiFi** (scan and connect).
- Configure **channels**: Telegram, Feishu, DingTalk, WeCom, QQ Channel, Webhook (tokens, keys, toggles).
- Configure **LLM**: API key, model, provider, compatible API URL (e.g. Ollama).
- Set **proxy**, **search keys**, and related options.
- View **system info**, **restart**, **OTA** (if enabled), **factory reset**.

All write operations require the correct pairing code; the UI sends it for you.

---

## For end users: how to use

### Prerequisites (required before using the config page)

You must have both of the following:

1. **A device with Beetle firmware flashed.**  
   The config page only talks to a device running the Beetle (甲虫) firmware. If you have not flashed the firmware yet, build and flash it first (see the **parent repo’s README or docs** for build and flash instructions). This UI does not replace the need for a flashed device.

2. **The device powered on and reachable.**  
   - **First use / not yet on your WiFi:** The device will open a **WiFi hotspot** with SSID **Beetle** (no password). Your phone or PC must **connect to this hotspot**; then open **http://192.168.4.1** (matches firmware SoftAP address).
   - **After WiFi is configured:** The device joins your router. Your phone or PC must be on the **same LAN** as the device; use the router-assigned device IP.

**Important:** Whether you open the config page from the device URL or from the GitHub Pages URL, your browser must be on the same network as the device (Beetle hotspot or same LAN). Otherwise the page cannot talk to the device.

---

### Option A – Open the config page from the device (direct)

You use the device’s own address in the browser; the device serves the UI (or redirects to it).

**When the device is not yet on your WiFi (first use):**

1. Power on the device → it opens hotspot **Beetle** (no password).
2. On your phone or PC, **connect to the WiFi “Beetle”**.
3. In the browser open **http://192.168.4.1** (port 80; firmware SoftAP address).

Only the device is on that hotspot; the firmware uses 192.168.4.1 for the SoftAP.

**When the device is already on your WiFi:**

- From any device on the **same LAN**, use the router-assigned device IP.

**First time on the config page:** Set a **6-digit pairing code**. It protects all write operations; secrets stay on the device. If you forget it, use **Factory reset** from the config page (you must still be able to open the page).

---

### Option B – Open the config page from the GitHub (online) URL

You open the **same UI** from the internet (e.g. **https://ai-orangeoracle.github.io/beetle/**). To **actually configure a device**, you still need a flashed device and your browser must be able to reach it (same network as above). The online page does **not** remove the need to flash the firmware or connect to the device’s network.

**Step-by-step when using the Git (GitHub Pages) address:**

1. **Prepare the device**  
   - Flash Beetle firmware to your hardware (see parent repo docs if needed).  
   - Power on the device.

2. **Put your phone or PC on the same network as the device**  
   - **Not yet on WiFi:** Connect your phone/PC to the device’s hotspot **Beetle** (no password).  
   - **Already on WiFi:** Ensure your phone/PC and the device are on the same LAN (e.g. same home/office router).

3. **Open the online config page**  
   - In the browser go to: **https://ai-orangeoracle.github.io/beetle/**  
   - (Or the repo’s custom domain if one is configured.)

4. **Enter the device address in the page**  
   - In the config UI, find the **“Device URL”** (设备地址) field.  
   - When connected to the device’s hotspot, enter **http://192.168.4.1** (firmware SoftAP address); when on the same LAN, enter the router-assigned IP.
   - Save. The page will then talk to the device at that address.

5. **Set pairing code and configure**  
   - On first use, set a **6-digit pairing code** on the config page.  
   - After that, you can configure WiFi, channels, LLM, etc. All write operations use this code (the UI sends it for you).

**If the page says it cannot reach the device:** Check that (1) the device is powered on, (2) you are connected to the **Beetle** hotspot or the **same LAN** as the device, and (3) the device address you entered is correct—use **http://192.168.4.1** when on the hotspot, or the device’s LAN IP when on the same LAN.

**Using the online URL only to preview:** You can open the GitHub Pages URL without a device to see the UI; to actually read or change config, you must have a device and be on its network as above.

---

### First-time setup and pairing code

- **First access:** Set a **6-digit pairing code** on the config page. It protects save/restart/OTA/factory reset; secrets are stored on the device only.
- **Forgot the code:** Use **Factory reset** from the config page (you must still be able to open the page and run the action).

More detail (config keys, health API, provisioning): see the parent repo’s **docs** directory (e.g. `docs/en-us/configuration.md`).

---

## For developers: how to use

### Prerequisites

- Node.js 20+  
- npm

### Commands

| Command          | Description                |
|------------------|----------------------------|
| `npm ci`         | Install dependencies       |
| `npm run dev`    | Start dev server           |
| `npm run build`  | TypeScript + Vite build    |
| `npm run lint`   | Run ESLint                 |
| `npm run preview`| Preview production build   |

### Local development

```bash
cd configure-ui
npm ci
npm run dev
```

Dev server runs with base path `/`; the app will call the device API at the host you use (e.g. after connecting to the device hotspot, use that machine’s browser so the same host is the device). For production (e.g. GitHub Pages), build uses `VITE_BASE_PATH=/<repo>/` so assets load correctly.

### Design and style

- UI and style rules (tokens, layout, no hardcoded colors): **docs/DESIGN.md**.
- Follow the design constraints when adding or changing UI.

### Deployment

A built version is published to GitHub Pages on push to `main` when `configure-ui/**` or the Pages workflow changes. One-time setup: in the repo **Settings → Pages**, set **Source** to **GitHub Actions**. To use a custom domain, set it in **Settings → Pages → Custom domain** and add the required DNS record (CNAME to `<owner>.github.io` for a subdomain, or A records for apex).
