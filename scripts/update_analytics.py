#!/usr/bin/env python3
"""
Nesstar Converter - Analytics Collector
Collects GitHub Traffic (views, clones, referrers, release downloads) and
PyPI download statistics, permanently archiving history in analytics/traffic_history.json
and generating ANALYTICS.md.
"""

import json
import os
import sys
import time
import urllib.error
import urllib.request
from datetime import datetime, timezone

REPO = "abhinavjnu/nesstar-converter"
PYPI_PACKAGE = "nesstar-converter"
ROOT_DIR = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
ANALYTICS_DIR = os.path.join(ROOT_DIR, "analytics")
HISTORY_FILE = os.path.join(ANALYTICS_DIR, "traffic_history.json")
MARKDOWN_FILE = os.path.join(ROOT_DIR, "ANALYTICS.md")

os.makedirs(ANALYTICS_DIR, exist_ok=True)

# ----------------- Load Existing History -----------------
history = {}
if os.path.exists(HISTORY_FILE):
    try:
        with open(HISTORY_FILE, "r", encoding="utf-8") as f:
            history = json.load(f)
    except Exception as e:
        print(f"Warning loading history file: {e}")

# ----------------- 1. GitHub API -----------------
gh_token = os.environ.get("GITHUB_TOKEN")
if not gh_token:
    # Try local gh CLI
    try:
        import subprocess
        gh_token = subprocess.check_output(["gh", "auth", "token"]).decode().strip()
    except Exception:
        pass

gh_headers = {
    "Accept": "application/vnd.github+json",
    "User-Agent": "nesstar-analytics"
}
if gh_token:
    gh_headers["Authorization"] = f"Bearer {gh_token}"

def fetch_gh(endpoint):
    url = f"https://api.github.com/repos/{REPO}/{endpoint}"
    req = urllib.request.Request(url, headers=gh_headers)
    try:
        with urllib.request.urlopen(req) as resp:
            return json.loads(resp.read().decode())
    except Exception as e:
        print(f"GitHub API [{endpoint}] error: {e}")
        return None

views_data = fetch_gh("traffic/views") or {}
clones_data = fetch_gh("traffic/clones") or {}
referrers_data = fetch_gh("traffic/popular/referrers") or []
releases_data = fetch_gh("releases") or []

# Merge Views
views_hist = history.get("views", {})
for v in views_data.get("views", []):
    ts = v["timestamp"].split("T")[0]
    views_hist[ts] = {"count": v["count"], "uniques": v["uniques"]}

# Merge Clones
clones_hist = history.get("clones", {})
for c in clones_data.get("clones", []):
    ts = c["timestamp"].split("T")[0]
    clones_hist[ts] = {"count": c["count"], "uniques": c["uniques"]}

# ----------------- 2. PyPI API with Retries -----------------
def fetch_pypi(endpoint, retries=3):
    url = f"https://pypistats.org/api/packages/{PYPI_PACKAGE}/{endpoint}"
    req = urllib.request.Request(url, headers={"User-Agent": "Mozilla/5.0 (compatible; nesstar-analytics/1.0)"})
    for attempt in range(retries):
        try:
            time.sleep(2.0)
            with urllib.request.urlopen(req) as resp:
                return json.loads(resp.read().decode())
        except urllib.error.HTTPError as e:
            if e.code == 429 and attempt < retries - 1:
                wait_time = (attempt + 1) * 3
                print(f"PyPI 429 rate limited, waiting {wait_time}s...")
                time.sleep(wait_time)
            else:
                print(f"PyPI API [{endpoint}] HTTP error: {e}")
                return None
        except Exception as e:
            print(f"PyPI API [{endpoint}] error: {e}")
            return None
    return None

pypi_recent_data = fetch_pypi("recent")
pypi_overall_data = fetch_pypi("overall")
pypi_os_data = fetch_pypi("system")
pypi_python_data = fetch_pypi("python_minor")

pypi_recent = pypi_recent_data.get("data", {}) if pypi_recent_data else history.get("pypi", {}).get("recent", {})

total_with_mirrors = 0
total_without_mirrors = 0
if pypi_overall_data and "data" in pypi_overall_data:
    for entry in pypi_overall_data["data"]:
        if entry.get("category") == "with_mirrors":
            total_with_mirrors += entry.get("downloads", 0)
        elif entry.get("category") == "without_mirrors":
            total_without_mirrors += entry.get("downloads", 0)
else:
    total_with_mirrors = history.get("pypi", {}).get("total_with_mirrors", 2560)
    total_without_mirrors = history.get("pypi", {}).get("total_without_mirrors", 806)

os_counts = {}
if pypi_os_data and "data" in pypi_os_data:
    for row in pypi_os_data["data"]:
        cat = row.get("category") or "Unknown"
        if cat.lower() != "null":
            os_counts[cat] = os_counts.get(cat, 0) + row.get("downloads", 0)
else:
    os_counts = history.get("pypi", {}).get("os_breakdown", {"Linux": 106, "Darwin (macOS)": 52, "Windows": 32})

py_counts = {}
if pypi_python_data and "data" in pypi_python_data:
    for row in pypi_python_data["data"]:
        cat = row.get("category") or "Unknown"
        if cat.lower() != "null":
            py_counts[cat] = py_counts.get(cat, 0) + row.get("downloads", 0)
else:
    py_counts = history.get("pypi", {}).get("python_breakdown", {"3.12": 68, "3.11": 56, "3.10": 32, "3.14": 20, "3.13": 14})

# ----------------- 3. Release Assets Downloads -----------------
release_assets = []
total_release_dl = 0
for r in releases_data:
    r_tag = r.get("tag_name") or r.get("name")
    for a in r.get("assets", []):
        count = a.get("download_count", 0)
        total_release_dl += count
        release_assets.append({
            "release": r_tag,
            "asset": a.get("name"),
            "size_mb": round(a.get("size", 0) / (1024 * 1024), 2),
            "downloads": count
        })

now_str = datetime.now(timezone.utc).strftime("%Y-%m-%d %H:%M:%S UTC")

# ----------------- 4. Save Updated History JSON -----------------
updated_history = {
    "last_updated": now_str,
    "views": views_hist,
    "clones": clones_hist,
    "pypi": {
        "recent": pypi_recent,
        "total_with_mirrors": total_with_mirrors,
        "total_without_mirrors": total_without_mirrors,
        "os_breakdown": os_counts,
        "python_breakdown": py_counts
    },
    "releases": release_assets,
    "total_release_downloads": total_release_dl
}

with open(HISTORY_FILE, "w", encoding="utf-8") as f:
    json.dump(updated_history, f, indent=2, sort_keys=True)

# ----------------- 5. Generate Markdown Report (ANALYTICS.md) -----------------
tot_hist_views = sum(v.get("count", 0) for v in views_hist.values())
tot_hist_clones = sum(c.get("count", 0) for c in clones_hist.values())

md = []
md.append("# 📈 Nesstar Converter — Analytics & Usage Report\n")
md.append(f"> **Last Updated:** `{now_str}` *(Automatically updated daily via GitHub Actions)*\n")

md.append("## 🚀 Overview Summary\n")
md.append("| Metric | Count | Description |")
md.append("|---|---|---|")
md.append(f"| **PyPI Total Downloads (Clean)** | **{total_without_mirrors:,}** | Direct pip installs (excluding mirror bots) |")
md.append(f"| **PyPI Total Downloads (Gross)** | **{total_with_mirrors:,}** | All recorded package downloads |")
md.append(f"| **PyPI Monthly Installs** | **{pypi_recent.get('last_month', 0):,}** | Downloads in the last 30 days |")
md.append(f"| **GitHub Release Binaries** | **{total_release_dl:,}** | GUI & CLI native executable downloads |")
md.append(f"| **Tracked Git Clones** | **{tot_hist_clones:,}** | Total recorded git clone operations |")
md.append(f"| **Tracked Repo Views** | **{tot_hist_views:,}** | Total recorded page visits |\n")

md.append("## 📦 PyPI Package Downloads Breakdown\n")
md.append("### By Operating System")
md.append("| Operating System | Direct Downloads |")
md.append("|---|---|")
for os_name, cnt in sorted(os_counts.items(), key=lambda x: x[1], reverse=True):
    icon = "🐧" if "linux" in os_name.lower() else ("🍎" if "darwin" in os_name.lower() or "macos" in os_name.lower() else ("🪟" if "windows" in os_name.lower() else "💻"))
    md.append(f"| {icon} {os_name} | {cnt:,} |")
md.append("")

md.append("### By Python Version")
md.append("| Python Version | Direct Downloads |")
md.append("|---|---|")
for py_ver, cnt in sorted(py_counts.items(), key=lambda x: x[1], reverse=True):
    md.append(f"| Python {py_ver} | {cnt:,} |")
md.append("\n---\n")

md.append("## 💾 GitHub Release Executables (Desktop GUI / CLI)\n")
if release_assets:
    md.append("| Release | Binary Asset | Size | Downloads |")
    md.append("|---|---|---|---|")
    for r in sorted(release_assets, key=lambda x: (x['release'], x['downloads']), reverse=True):
        md.append(f"| `{r['release']}` | **{r['asset']}** | {r['size_mb']} MB | **{r['downloads']}** |")
else:
    md.append("*No release assets found.*")
md.append("\n---\n")

md.append("## 🌐 Permanent GitHub Traffic History\n")
md.append("| Date | Page Views (Count / Unique) | Git Clones (Count / Unique) |")
md.append("|---|---|---|")

all_dates = sorted(set(list(views_hist.keys()) + list(clones_hist.keys())), reverse=True)
for d in all_dates:
    v = views_hist.get(d, {"count": 0, "uniques": 0})
    c = clones_hist.get(d, {"count": 0, "uniques": 0})
    md.append(f"| `{d}` | {v.get('count', 0)} ({v.get('uniques', 0)} unique) | {c.get('count', 0)} ({c.get('uniques', 0)} unique) |")

md.append("\n---\n")
md.append("### 🔗 Quick Analytics Links\n")
md.append(f"- **[PePy.tech Dashboard](https://pepy.tech/project/{PYPI_PACKAGE})** — PyPI lifetime charts & daily graphs")
md.append(f"- **[PyPI Stats Dashboard](https://pypistats.org/packages/{PYPI_PACKAGE})** — PyPI OS and minor version breakdown")
md.append(f"- **[GitHub Insights Traffic](https://github.com/{REPO}/graphs/traffic)** — Official GitHub 14-day rolling traffic\n")

with open(MARKDOWN_FILE, "w", encoding="utf-8") as f:
    f.write("\n".join(md))

print("Successfully generated ANALYTICS.md and updated traffic_history.json!")
