# Agent tools

[中文](../zh-cn/tools.md) | **English**

This doc is for **users of a Beetle device**: it describes the **tools** the on-device AI Agent can use during a conversation, what each does, and the limits. You do not call these tools yourself—the Agent decides when to use one based on your message. If a call fails (e.g. network error, invalid args), the Agent reports it in natural language.

---

## Overview

| Tool | What it does | When the Agent might use it |
|------|----------------|-----------------------------|
| **get_time** | Returns current UTC time (date, weekday, time). | You ask for the time, today’s date, or when something should run. |
| **cron** | Parses a 5-field cron expression and returns the next run time in UTC. | You ask “when will this cron run?” or “next trigger time for …”. |
| **files** | Lists or reads files from device storage (SPIFFS). Paths are under the storage root; no `..` allowed. | You ask to list or read a file/folder (e.g. skills, config, memory). |
| **web_search** | Searches the web for a query and returns a short summary. | You ask for recent info, facts, or “search for …”. |
| **analyze_image** | Analyzes an image from a URL using vision AI. | You send an image URL and ask what’s in it or to describe it. |
| **fetch_url** | Fetches a URL with HTTP GET and returns the response body (text, truncated). Only http(s). | You ask to “open this link” or “get content from URL”. |
| **http_post** | Sends an HTTP POST to a URL with a body and returns the response. Only http(s). | You ask to trigger a webhook, call an API (e.g. Home Assistant, IFTTT, n8n), or push data somewhere. |
| **remind_at** | Schedules a reminder. At the given time you get a message with the reminder text. | You say “remind me at …” or “at 3pm tell me …”. |
| **kv_store** | Persistent key-value store (survives reboot). Operations: get, set, delete, list_keys. Keys: letters, numbers, `_`, `-`, `.`; max 64 chars. Values: max 512 bytes. Max 64 entries. | You ask to “remember X”, “save my preference”, “what did I set for …”, or “list what you’ve stored”. |
| **update_session_summary** | Writes a short summary of the conversation so far for future context. | The Agent uses it at natural breaks in long chats so it can refer back later. |
| **board_info** | Returns device status: chip model, free heap/PSRAM, uptime, IDF version, resource pressure, WiFi connected or not, SPIFFS storage (total/used/free). | You ask “device status”, “how much memory”, “is WiFi connected”, “storage space”, or “what chip”. |

**Optional (feature `gpio`):**

| Tool | What it does | When the Agent might use it |
|------|----------------|-----------------------------|
| **gpio_read** | Reads GPIO pin level (0 or 1). Only pins 2 and 13 are allowed. | You ask to “read pin X” or “is the pin high”. |
| **gpio_write** | Sets GPIO pin output level (0 or 1). Only pins 2 and 13. | You ask to “set pin X high/low” or “turn on/off pin”. |

---

## Limits and behavior

- **Time**: On the device, time is only correct after NTP/RTC sync. Use **get_time** to check.
- **Storage (files)**: Read-only for the files tool; paths must not leave the storage root. List returns at most 256 entries; read content is truncated.
- **Reminders**: Stored on device; at the set time a message is sent to you in the same channel. Number of reminders is limited.
- **Web / HTTP**: Under low memory the Agent may avoid or delay network tools (web_search, fetch_url, http_post, analyze_image) to keep the device stable.
- **board_info**: Gives a snapshot of memory, pressure, WiFi, and SPIFFS so you can see if the device is healthy or under load.

To control on-board hardware (GPIO, PWM, buzzer, etc.) via config and have the Agent call it by meaning, see [Hardware device config](hardware-device-config.md) and the “Hardware” section in the config page.
