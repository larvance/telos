#!/usr/bin/env python3
"""
Telos News Portal & RTC Content Management System
Powers news.telos.larvance.com and the desktop launcher news feed.
Supports local live-editing, static site generation, and zero-config deployment to Vercel / GitHub Pages.
"""

import sys
import os
import json
import shutil
import http.server
import socketserver
import urllib.parse
import argparse
from pathlib import Path
from datetime import datetime

BASE_DIR = Path(__file__).resolve().parent
CONTENT_DIR = BASE_DIR / "content" / "articles"
ASSETS_DIR = BASE_DIR / "assets"
DIST_DIR = BASE_DIR / "dist"
CNAME_FILE = BASE_DIR / "CNAME"
VERCEL_CONFIG = BASE_DIR / "vercel.json"

DEFAULT_PORT = 3344
DEFAULT_DOMAIN = "news.telos.larvance.com"


def load_articles():
    """Load and sort all article JSON files by timestamp descending."""
    CONTENT_DIR.mkdir(parents=True, exist_ok=True)
    articles = []
    for file_path in sorted(CONTENT_DIR.glob("*.json")):
        try:
            with open(file_path, "r", encoding="utf-8") as f:
                data = json.load(f)
                data["_filename"] = file_path.name
                if "id" not in data:
                    data["id"] = file_path.stem
                articles.append(data)
        except Exception as e:
            print(f"Error loading {file_path.name}: {e}", file=sys.stderr)

    articles.sort(key=lambda a: a.get("timestamp", a.get("date", "")), reverse=True)
    return articles


def save_article(article_data):
    """Save an article JSON to content directory."""
    CONTENT_DIR.mkdir(parents=True, exist_ok=True)
    article_id = article_data.get("id")
    if not article_id:
        slug = article_data.get("title", "untitled").lower()
        slug = "".join(c if c.isalnum() else "-" for c in slug).strip("-")
        article_id = slug or f"post-{int(datetime.utcnow().timestamp())}"
        article_data["id"] = article_id

    filename = article_data.pop("_filename", f"{article_id}.json")
    if not filename.endswith(".json"):
        filename = f"{filename}.json"

    file_path = CONTENT_DIR / filename
    with open(file_path, "w", encoding="utf-8") as f:
        json.dump(article_data, f, indent=2, ensure_ascii=False)

    print(f"Saved article: {file_path.name}")
    return article_data


def delete_article(article_id):
    """Delete an article by id."""
    for file_path in CONTENT_DIR.glob("*.json"):
        try:
            with open(file_path, "r", encoding="utf-8") as f:
                data = json.load(f)
                if data.get("id") == article_id:
                    file_path.unlink()
                    print(f"Deleted article: {file_path.name}")
                    return True
        except Exception:
            pass
    return False


def build_site():
    """Build the static production site into dist/."""
    DIST_DIR.mkdir(parents=True, exist_ok=True)
    articles = load_articles()

    # 1. Output news.json feed for launcher & API
    clean_articles = []
    for a in articles:
        copy_a = dict(a)
        copy_a.pop("_filename", None)
        clean_articles.append(copy_a)

    with open(DIST_DIR / "news.json", "w", encoding="utf-8") as f:
        json.dump(clean_articles, f, indent=2, ensure_ascii=False)
    print(f"Generated {DIST_DIR / 'news.json'} ({len(clean_articles)} articles)")

    # 2. Copy assets (images, fonts, etc.)
    dist_assets = DIST_DIR / "assets"
    if dist_assets.exists():
        shutil.rmtree(dist_assets)
    if ASSETS_DIR.exists():
        shutil.copytree(ASSETS_DIR, dist_assets)

    # 3. Copy CNAME and vercel.json
    if CNAME_FILE.exists():
        shutil.copy2(CNAME_FILE, DIST_DIR / "CNAME")
    if VERCEL_CONFIG.exists():
        shutil.copy2(VERCEL_CONFIG, DIST_DIR / "vercel.json")

    # 4. Generate HTML portal (index.html)
    index_html = render_portal_html(clean_articles)
    with open(DIST_DIR / "index.html", "w", encoding="utf-8") as f:
        f.write(index_html)

    # 5. Generate individual article pages
    articles_dir = DIST_DIR / "articles"
    articles_dir.mkdir(parents=True, exist_ok=True)
    for art in clean_articles:
        art_html = render_article_html(art)
        with open(articles_dir / f"{art['id']}.html", "w", encoding="utf-8") as f:
            f.write(art_html)

    print(f"Build complete in {DIST_DIR}")


def render_portal_html(articles):
    featured = articles[0] if articles else None
    remaining = articles[1:] if len(articles) > 1 else []

    featured_html = ""
    if featured:
        featured_html = f"""
        <section class="featured-banner">
          <div class="featured-media" style="background-image: url('{featured.get("image", "")}');">
            <div class="featured-gradient"></div>
            <div class="featured-content">
              <span class="category-badge">{featured.get("category", "Announcement")}</span>
              <span class="featured-date">{featured.get("date", "")}</span>
              <h2 class="featured-title">{featured.get("title", "")}</h2>
              <p class="featured-summary">{featured.get("summary", "")}</p>
              <a href="/articles/{featured.get("id")}.html" class="btn-read-more">Read Full Story &rarr;</a>
            </div>
          </div>
        </section>
        """

    cards_html = ""
    for art in remaining:
        cards_html += f"""
        <article class="news-card" data-category="{art.get("category_slug", "all")}">
          <div class="card-thumb-wrap">
            <img src="/{art.get("image", "assets/images/underwater_overhaul.jpg")}" alt="{art.get("title", "")}" class="card-thumb" loading="lazy">
            <span class="card-badge">{art.get("category", "Update")}</span>
          </div>
          <div class="card-body">
            <span class="card-date">{art.get("date", "")}</span>
            <h3 class="card-title"><a href="/articles/{art.get("id")}.html">{art.get("title", "")}</a></h3>
            <p class="card-summary">{art.get("summary", "")}</p>
            <div class="card-footer">
              <span class="card-author">By {art.get("author", "Telos Team")}</span>
              <a href="/articles/{art.get("id")}.html" class="card-link">Read &rarr;</a>
            </div>
          </div>
        </article>
        """

    return f"""<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>Telos News — Official Development Updates</title>
  <meta name="description" content="Official developer dispatches, changelogs, and announcements for Telos Voxel Engine.">
  <link rel="icon" type="image/png" href="https://telos.larvance.com/assets/telos.png">
  <style>
    :root {{
      --bg: #141414;
      --card-bg: #1e1e1e;
      --border: #2a2a2a;
      --text: #f0f0f0;
      --text-muted: #9e9e9e;
      --gold: #dfa824;
      --green: #3c8527;
      --green-light: #4ea833;
    }}
    * {{ box-sizing: border-box; margin: 0; padding: 0; }}
    body {{
      font-family: 'Ubuntu', -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
      background: var(--bg);
      color: var(--text);
      line-height: 1.6;
    }}
    header {{
      background: #181818;
      border-bottom: 1px solid var(--border);
      padding: 16px 32px;
      display: flex;
      justify-content: space-between;
      align-items: center;
      position: sticky;
      top: 0;
      z-index: 100;
    }}
    .brand {{
      display: flex;
      align-items: center;
      gap: 12px;
      text-decoration: none;
      color: #fff;
      font-weight: 700;
      font-size: 18px;
      letter-spacing: 1px;
    }}
    .brand-cube {{
      width: 22px;
      height: 22px;
      background: var(--gold);
      border-radius: 4px;
      display: inline-block;
    }}
    .nav-links {{ display: flex; gap: 20px; align-items: center; }}
    .nav-links a {{ color: var(--text-muted); text-decoration: none; font-size: 14px; font-weight: 500; transition: color 0.15s; }}
    .nav-links a:hover {{ color: #fff; }}
    .btn-play {{
      background: var(--green);
      color: #fff !important;
      padding: 8px 18px;
      border-radius: 4px;
      box-shadow: inset 0 1px 0 rgba(255,255,255,0.3), 0 3px 0 var(--green-light);
      font-weight: 700 !important;
    }}
    .container {{ max-width: 1200px; margin: 0 auto; padding: 32px 24px; }}
    .featured-banner {{
      position: relative;
      border-radius: 12px;
      overflow: hidden;
      margin-bottom: 48px;
      border: 1px solid var(--border);
      box-shadow: 0 12px 32px rgba(0,0,0,0.6);
    }}
    .featured-media {{
      height: 480px;
      background-size: cover;
      background-position: center;
      position: relative;
      display: flex;
      align-items: flex-end;
    }}
    .featured-gradient {{
      position: absolute;
      top: 0; left: 0; right: 0; bottom: 0;
      background: linear-gradient(180deg, rgba(0,0,0,0.1) 0%, rgba(20,20,20,0.95) 100%);
    }}
    .featured-content {{
      position: relative;
      z-index: 2;
      padding: 40px;
      max-width: 800px;
    }}
    .category-badge {{
      display: inline-block;
      background: var(--gold);
      color: #000;
      font-weight: 700;
      font-size: 11px;
      padding: 4px 10px;
      border-radius: 3px;
      text-transform: uppercase;
      letter-spacing: 0.8px;
      margin-bottom: 12px;
    }}
    .featured-date {{ font-size: 13px; color: var(--text-muted); margin-left: 12px; }}
    .featured-title {{ font-size: 32px; font-weight: 800; color: #fff; margin-bottom: 12px; line-height: 1.25; }}
    .featured-summary {{ font-size: 16px; color: #ccc; margin-bottom: 20px; }}
    .btn-read-more {{
      display: inline-block;
      background: var(--green);
      color: #fff;
      text-decoration: none;
      padding: 10px 22px;
      border-radius: 4px;
      font-weight: 700;
      box-shadow: 0 4px 12px rgba(60,133,39,0.4);
      transition: background 0.15s;
    }}
    .btn-read-more:hover {{ background: var(--green-light); }}
    .section-header {{
      display: flex;
      justify-content: space-between;
      align-items: center;
      margin-bottom: 24px;
      border-bottom: 1px solid var(--border);
      padding-bottom: 12px;
    }}
    .section-title {{ font-size: 22px; font-weight: 700; }}
    .news-grid {{
      display: grid;
      grid-template-columns: repeat(auto-fill, minmax(340px, 1fr));
      gap: 28px;
    }}
    .news-card {{
      background: var(--card-bg);
      border: 1px solid var(--border);
      border-radius: 8px;
      overflow: hidden;
      display: flex;
      flex-direction: column;
      transition: transform 0.2s, box-shadow 0.2s, border-color 0.2s;
    }}
    .news-card:hover {{
      transform: translateY(-4px);
      box-shadow: 0 10px 24px rgba(0,0,0,0.5);
      border-color: #444;
    }}
    .card-thumb-wrap {{ position: relative; height: 200px; overflow: hidden; }}
    .card-thumb {{ width: 100%; height: 100%; object-fit: cover; transition: transform 0.3s; }}
    .news-card:hover .card-thumb {{ transform: scale(1.04); }}
    .card-badge {{
      position: absolute;
      top: 12px;
      left: 12px;
      background: rgba(0,0,0,0.75);
      backdrop-filter: blur(4px);
      border: 1px solid rgba(255,255,255,0.2);
      color: #fff;
      font-size: 10px;
      font-weight: 700;
      padding: 3px 8px;
      border-radius: 3px;
      text-transform: uppercase;
    }}
    .card-body {{ padding: 20px; display: flex; flex-direction: column; flex: 1; }}
    .card-date {{ font-size: 12px; color: var(--text-muted); margin-bottom: 6px; }}
    .card-title {{ font-size: 18px; font-weight: 700; margin-bottom: 10px; line-height: 1.35; }}
    .card-title a {{ color: #fff; text-decoration: none; transition: color 0.15s; }}
    .card-title a:hover {{ color: var(--gold); }}
    .card-summary {{ font-size: 13px; color: #aaa; flex: 1; margin-bottom: 16px; }}
    .card-footer {{ display: flex; justify-content: space-between; align-items: center; border-top: 1px solid var(--border); padding-top: 12px; font-size: 12px; }}
    .card-author {{ color: var(--text-muted); }}
    .card-link {{ color: var(--gold); text-decoration: none; font-weight: 600; }}
    footer {{
      background: #101010;
      border-top: 1px solid var(--border);
      padding: 40px 24px;
      text-align: center;
      color: var(--text-muted);
      font-size: 13px;
      margin-top: 60px;
    }}
    @media (max-width: 768px) {{
      .featured-media {{ height: 360px; }}
      .featured-content {{ padding: 20px; }}
      .featured-title {{ font-size: 24px; }}
    }}
  </style>
</head>
<body>
  <header>
    <a href="/" class="brand">
      <span class="brand-cube"></span>
      <span>TELOS NEWS</span>
    </a>
    <nav class="nav-links">
      <a href="https://telos.larvance.com" target="_blank" rel="noopener">Play Web Port</a>
      <a href="https://github.com/larvance/telos" target="_blank" rel="noopener">GitHub</a>
      <a href="/news.json" target="_blank" class="btn-play">API Feed</a>
    </nav>
  </header>

  <main class="container">
    {featured_html}

    <div class="section-header">
      <h3 class="section-title">Latest Announcements</h3>
      <span style="color: var(--text-muted); font-size: 13px;">Auto-synchronized with launcher</span>
    </div>

    <div class="news-grid">
      {cards_html}
    </div>
  </main>

  <footer>
    <p>&copy; 2026 Telos Development Team &bull; <a href="https://larvance.com" style="color: var(--gold); text-decoration: none;">larvance.com</a> &bull; Hosted on Vercel / GitHub Pages</p>
  </footer>
</body>
</html>
"""


def render_article_html(art):
    return f"""<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>{art.get("title", "")} — Telos News</title>
  <meta name="description" content="{art.get("summary", "")}">
  <style>
    :root {{
      --bg: #141414;
      --card-bg: #1e1e1e;
      --border: #2a2a2a;
      --text: #f0f0f0;
      --text-muted: #9e9e9e;
      --gold: #dfa824;
      --green: #3c8527;
    }}
    * {{ box-sizing: border-box; margin: 0; padding: 0; }}
    body {{
      font-family: 'Ubuntu', -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
      background: var(--bg);
      color: var(--text);
      line-height: 1.7;
    }}
    header {{
      background: #181818;
      border-bottom: 1px solid var(--border);
      padding: 16px 32px;
      display: flex;
      justify-content: space-between;
      align-items: center;
    }}
    .brand {{ text-decoration: none; color: #fff; font-weight: 700; font-size: 18px; }}
    .container {{ max-width: 860px; margin: 0 auto; padding: 40px 24px; }}
    .back-link {{ color: var(--gold); text-decoration: none; font-size: 14px; font-weight: 600; display: inline-block; margin-bottom: 24px; }}
    .article-header {{ margin-bottom: 32px; }}
    .category-badge {{
      background: var(--gold);
      color: #000;
      font-weight: 700;
      font-size: 11px;
      padding: 4px 10px;
      border-radius: 3px;
      text-transform: uppercase;
      display: inline-block;
      margin-bottom: 12px;
    }}
    .article-title {{ font-size: 36px; font-weight: 800; line-height: 1.25; margin-bottom: 12px; }}
    .article-meta {{ font-size: 14px; color: var(--text-muted); }}
    .article-hero {{
      width: 100%;
      height: 420px;
      object-fit: cover;
      border-radius: 8px;
      margin-bottom: 32px;
      border: 1px solid var(--border);
    }}
    .article-content {{ font-size: 17px; color: #d0d0d0; }}
    .article-content p {{ margin-bottom: 20px; }}
    footer {{
      background: #101010;
      border-top: 1px solid var(--border);
      padding: 30px;
      text-align: center;
      color: var(--text-muted);
      font-size: 13px;
      margin-top: 80px;
    }}
  </style>
</head>
<body>
  <header>
    <a href="/" class="brand">TELOS NEWS</a>
    <a href="/" style="color: var(--text-muted); text-decoration: none; font-size: 14px;">&larr; Back to News</a>
  </header>
  <main class="container">
    <a href="/" class="back-link">&larr; Back to All Updates</a>
    <article>
      <header class="article-header">
        <span class="category-badge">{art.get("category", "Update")}</span>
        <h1 class="article-title">{art.get("title", "")}</h1>
        <div class="article-meta">
          <span>Published on {art.get("date", "")}</span> &bull; <span>By {art.get("author", "Telos Team")}</span>
        </div>
      </header>
      <img src="/{art.get("image", "")}" alt="{art.get("title", "")}" class="article-hero">
      <div class="article-content">
        <p><strong>{art.get("summary", "")}</strong></p>
        <p>{art.get("content", "")}</p>
      </div>
    </article>
  </main>
  <footer>
    <p>&copy; 2026 Telos Development Team &bull; <a href="https://news.telos.larvance.com" style="color: var(--gold); text-decoration: none;">news.telos.larvance.com</a></p>
  </footer>
</body>
</html>
"""


class CMSHandler(http.server.SimpleHTTPRequestHandler):
    """Real-Time Content (RTC) HTTP & API Server."""

    def __init__(self, *args, **kwargs):
        super().__init__(*args, directory=str(DIST_DIR), **kwargs)

    def do_GET(self):
        url = urllib.parse.urlparse(self.path)

        # Studio CMS UI
        if url.path in ("/admin", "/studio", "/editor"):
            self.send_response(200)
            self.send_header("Content-Type", "text/html; charset=utf-8")
            self.end_headers()
            self.wfile.write(self.render_editor_ui().encode("utf-8"))
            return

        # API: list news
        if url.path == "/api/news":
            self.send_response(200)
            self.send_header("Content-Type", "application/json")
            self.send_header("Access-Control-Allow-Origin", "*")
            self.end_headers()
            articles = load_articles()
            self.wfile.write(json.dumps(articles).encode("utf-8"))
            return

        super().do_GET()

    def do_POST(self):
        url = urllib.parse.urlparse(self.path)

        # API: save article
        if url.path == "/api/news":
            content_len = int(self.headers.get("Content-Length", 0))
            post_body = self.rfile.read(content_len)
            try:
                data = json.loads(post_body.decode("utf-8"))
                saved = save_article(data)
                build_site()
                self.send_response(200)
                self.send_header("Content-Type", "application/json")
                self.send_header("Access-Control-Allow-Origin", "*")
                self.end_headers()
                self.wfile.write(json.dumps({"success": True, "article": saved}).encode("utf-8"))
            except Exception as e:
                self.send_response(400)
                self.end_headers()
                self.wfile.write(json.dumps({"error": str(e)}).encode("utf-8"))
            return

        # API: deploy
        if url.path == "/api/deploy":
            content_len = int(self.headers.get("Content-Length", 0))
            post_body = self.rfile.read(content_len)
            target = "vercel"
            if post_body:
                try:
                    payload = json.loads(post_body.decode("utf-8"))
                    target = payload.get("target", "vercel")
                except Exception:
                    pass

            res = run_deploy(target)
            self.send_response(200 if res.get("success") else 500)
            self.send_header("Content-Type", "application/json")
            self.send_header("Access-Control-Allow-Origin", "*")
            self.end_headers()
            self.wfile.write(json.dumps(res).encode("utf-8"))
            return

        super().do_POST()

    def do_DELETE(self):
        url = urllib.parse.urlparse(self.path)
        if url.path.startswith("/api/news/"):
            art_id = url.path.split("/")[-1]
            success = delete_article(art_id)
            if success:
                build_site()
                self.send_response(200)
                self.send_header("Content-Type", "application/json")
                self.end_headers()
                self.wfile.write(json.dumps({"success": True}).encode("utf-8"))
            else:
                self.send_response(404)
                self.end_headers()
            return
        super().do_DELETE()

    def do_OPTIONS(self):
        self.send_response(200)
        self.send_header("Access-Control-Allow-Origin", "*")
        self.send_header("Access-Control-Allow-Methods", "GET, POST, DELETE, OPTIONS")
        self.send_header("Access-Control-Allow-Headers", "Content-Type")
        self.end_headers()

    def render_editor_ui(self):
        articles = load_articles()
        return f"""<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <title>Telos News Studio — RTC CMS</title>
  <style>
    :root {{
      --bg: #121212; --panel: #1e1e1e; --border: #333; --gold: #dfa824; --green: #3c8527;
      --text: #eee; --muted: #888;
    }}
    * {{ box-sizing: border-box; margin: 0; padding: 0; }}
    body {{ font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif; background: var(--bg); color: var(--text); display: flex; height: 100vh; overflow: hidden; }}
    #sidebar {{ width: 340px; background: var(--panel); border-right: 1px solid var(--border); display: flex; flex-direction: column; }}
    .sidebar-header {{ padding: 16px; border-bottom: 1px solid var(--border); display: flex; justify-content: space-between; align-items: center; }}
    .btn {{ background: #2a2a2a; color: #fff; border: 1px solid var(--border); padding: 6px 12px; border-radius: 4px; cursor: pointer; font-size: 13px; }}
    .btn:hover {{ background: #383838; }}
    .btn-green {{ background: var(--green); border: none; font-weight: 700; }}
    .btn-green:hover {{ background: #4ea833; }}
    .btn-gold {{ background: var(--gold); color: #000; border: none; font-weight: 700; }}
    .article-list {{ flex: 1; overflow-y: auto; list-style: none; }}
    .article-item {{ padding: 12px 16px; border-bottom: 1px solid var(--border); cursor: pointer; transition: background 0.15s; }}
    .article-item:hover, .article-item.active {{ background: #262626; }}
    .article-item-title {{ font-size: 14px; font-weight: 600; margin-bottom: 4px; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }}
    .article-item-meta {{ font-size: 11px; color: var(--muted); }}
    #main {{ flex: 1; display: flex; flex-direction: column; overflow: hidden; }}
    .top-bar {{ height: 50px; background: var(--panel); border-bottom: 1px solid var(--border); display: flex; align-items: center; justify-content: space-between; padding: 0 24px; }}
    .editor-grid {{ flex: 1; display: grid; grid-template-columns: 1fr 1fr; gap: 20px; padding: 24px; overflow-y: auto; }}
    .form-group {{ margin-bottom: 16px; }}
    label {{ display: block; font-size: 12px; color: var(--muted); margin-bottom: 6px; font-weight: 600; text-transform: uppercase; }}
    input, textarea, select {{ width: 100%; background: #181818; border: 1px solid var(--border); color: #fff; padding: 10px; border-radius: 4px; font-size: 14px; }}
    textarea {{ height: 160px; font-family: inherit; resize: vertical; }}
    .preview-card {{ background: var(--panel); border: 1px solid var(--border); border-radius: 8px; overflow: hidden; max-width: 440px; margin-top: 10px; }}
    .preview-thumb {{ width: 100%; height: 200px; object-fit: cover; }}
    .preview-body {{ padding: 16px; }}
    .preview-badge {{ background: var(--gold); color: #000; font-size: 10px; font-weight: 700; padding: 2px 6px; border-radius: 2px; text-transform: uppercase; }}
    .preview-title {{ font-size: 16px; font-weight: 700; margin: 8px 0; }}
    .preview-summary {{ font-size: 12px; color: #aaa; }}
  </style>
</head>
<body>
  <div id="sidebar">
    <div class="sidebar-header">
      <h3 style="font-size: 15px;">Telos News CMS</h3>
      <button class="btn btn-green" id="btn-new-post">+ New Post</button>
    </div>
    <ul class="article-list" id="article-list"></ul>
    <div style="padding: 12px; border-top: 1px solid var(--border); display: flex; gap: 8px;">
      <button class="btn btn-gold" id="btn-deploy-vercel" style="flex: 1;">Deploy to Vercel</button>
      <a href="/" target="_blank" class="btn" style="text-decoration: none; text-align: center;">View Site ↗</a>
    </div>
  </div>

  <div id="main">
    <div class="top-bar">
      <span id="editor-status" style="font-size: 13px; color: var(--muted);">Select or create an article</span>
      <div style="display: flex; gap: 8px;">
        <button class="btn" id="btn-delete" style="color: #ff5555; display: none;">Delete</button>
        <button class="btn btn-green" id="btn-save">Save & Rebuild</button>
      </div>
    </div>

    <div class="editor-grid">
      <div class="form-pane">
        <div class="form-group">
          <label>Title</label>
          <input type="text" id="inp-title" placeholder="e.g. Major Engine Update">
        </div>
        <div class="form-group">
          <label>Slug / ID</label>
          <input type="text" id="inp-id" placeholder="e.g. major-engine-update">
        </div>
        <div class="form-group">
          <label>Category</label>
          <select id="inp-category">
            <option value="Graphics & Shading">Graphics & Shading</option>
            <option value="Engine Architecture">Engine Architecture</option>
            <option value="Multiplayer">Multiplayer</option>
            <option value="Worldgen">Worldgen</option>
            <option value="Announcement">Announcement</option>
          </select>
        </div>
        <div class="form-group">
          <label>Featured Image Path</label>
          <select id="inp-image">
            <option value="assets/images/underwater_overhaul.jpg">Underwater Overhaul</option>
            <option value="assets/images/dual_gpu_pipeline.jpg">Dual GPU Pipeline</option>
            <option value="assets/images/multiplayer_netcode.jpg">Multiplayer Town</option>
            <option value="assets/images/caves_worldgen.jpg">Caverns & Aquifers</option>
          </select>
        </div>
        <div class="form-group">
          <label>Summary / Excerpt</label>
          <textarea id="inp-summary" style="height: 80px;" placeholder="Short summary for card view..."></textarea>
        </div>
        <div class="form-group">
          <label>Full Content</label>
          <textarea id="inp-content" placeholder="Full story markdown or prose..."></textarea>
        </div>
      </div>

      <div class="preview-pane">
        <label>Live Card Preview</label>
        <div class="preview-card">
          <img src="" id="prev-img" class="preview-thumb">
          <div class="preview-body">
            <span class="preview-badge" id="prev-badge">Category</span>
            <h4 class="preview-title" id="prev-title">Article Title</h4>
            <p class="preview-summary" id="prev-summary">Short description...</p>
          </div>
        </div>
      </div>
    </div>
  </div>

  <script>
    let articles = {json.dumps(articles)};
    let currentId = null;

    function renderList() {{
      const ul = document.getElementById('article-list');
      ul.innerHTML = '';
      articles.forEach(a => {{
        const li = document.createElement('li');
        li.className = 'article-item' + (a.id === currentId ? ' active' : '');
        li.innerHTML = `
          <div class="article-item-title">${{a.title}}</div>
          <div class="article-item-meta">${{a.date || ''}} &bull; ${{a.category || ''}}</div>
        `;
        li.onclick = () => loadArticle(a.id);
        ul.appendChild(li);
      }});
    }}

    function loadArticle(id) {{
      currentId = id;
      const a = articles.find(x => x.id === id);
      if (!a) return;
      document.getElementById('inp-title').value = a.title || '';
      document.getElementById('inp-id').value = a.id || '';
      document.getElementById('inp-category').value = a.category || 'Announcement';
      document.getElementById('inp-image').value = a.image || 'assets/images/underwater_overhaul.jpg';
      document.getElementById('inp-summary').value = a.summary || '';
      document.getElementById('inp-content').value = a.content || '';
      document.getElementById('btn-delete').style.display = 'inline-block';
      document.getElementById('editor-status').textContent = 'Editing: ' + a.title;
      updatePreview();
      renderList();
    }}

    function updatePreview() {{
      document.getElementById('prev-title').textContent = document.getElementById('inp-title').value || 'Untitled';
      document.getElementById('prev-badge').textContent = document.getElementById('inp-category').value;
      document.getElementById('prev-summary').textContent = document.getElementById('inp-summary').value || 'No summary entered.';
      document.getElementById('prev-img').src = '/' + document.getElementById('inp-image').value;
    }}

    ['inp-title', 'inp-category', 'inp-summary', 'inp-image'].forEach(id => {{
      document.getElementById(id).addEventListener('input', updatePreview);
    }});

    document.getElementById('btn-new-post').onclick = () => {{
      currentId = null;
      document.getElementById('inp-title').value = '';
      document.getElementById('inp-id').value = 'new-post-' + Date.now();
      document.getElementById('inp-summary').value = '';
      document.getElementById('inp-content').value = '';
      document.getElementById('btn-delete').style.display = 'none';
      document.getElementById('editor-status').textContent = 'Drafting new article';
      updatePreview();
      renderList();
    }};

    document.getElementById('btn-save').onclick = async () => {{
      const post = {{
        id: document.getElementById('inp-id').value.trim(),
        title: document.getElementById('inp-title').value.trim(),
        category: document.getElementById('inp-category').value,
        category_slug: document.getElementById('inp-category').value.toLowerCase().replace(/[^a-z0-9]+/g, '-'),
        image: document.getElementById('inp-image').value,
        summary: document.getElementById('inp-summary').value.trim(),
        content: document.getElementById('inp-content').value.trim(),
        date: new Date().toLocaleDateString('en-US', {{ month: 'long', day: 'numeric', year: 'numeric' }}),
        timestamp: new Date().toISOString(),
        author: 'Telos Dev Team'
      }};

      const res = await fetch('/api/news', {{
        method: 'POST',
        headers: {{ 'Content-Type': 'application/json' }},
        body: JSON.stringify(post)
      }});
      if (res.ok) {{
        alert('Article saved and static site rebuilt!');
        const updated = await fetch('/api/news').then(r => r.json());
        articles = updated;
        currentId = post.id;
        renderList();
      }} else {{
        alert('Failed to save article.');
      }}
    }};

    document.getElementById('btn-delete').onclick = async () => {{
      if (!currentId || !confirm('Delete article ' + currentId + '?')) return;
      const res = await fetch('/api/news/' + currentId, {{ method: 'DELETE' }});
      if (res.ok) {{
        articles = await fetch('/api/news').then(r => r.json());
        document.getElementById('btn-new-post').click();
      }}
    }};

    document.getElementById('btn-deploy-vercel').onclick = async () => {{
      const btn = document.getElementById('btn-deploy-vercel');
      btn.textContent = 'Deploying...';
      btn.disabled = true;
      try {{
        const res = await fetch('/api/deploy', {{
          method: 'POST',
          headers: {{ 'Content-Type': 'application/json' }},
          body: JSON.stringify({{ target: 'vercel' }})
        }});
        const json = await res.json();
        alert(json.message || 'Deployment triggered.');
      }} catch (err) {{
        alert('Deploy error: ' + err);
      }} finally {{
        btn.textContent = 'Deploy to Vercel';
        btn.disabled = false;
      }}
    }};

    if (articles.length > 0) {{
      loadArticle(articles[0].id);
    }} else {{
      document.getElementById('btn-new-post').click();
    }}
  </script>
</body>
</html>
"""


def run_deploy(target="vercel"):
    """Deploy the dist directory."""
    build_site()
    print(f"Deploying to {target}...")

    if target == "vercel":
        cmd = f"npx --yes vercel deploy --prod {DIST_DIR} --yes"
        token = os.environ.get("VERCEL_TOKEN")
        if token:
            cmd += f" --token {token}"
        print(f"Running command: {cmd}")
        ret = os.system(cmd)
        if ret == 0:
            return {"success": True, "message": f"Successfully deployed to {DEFAULT_DOMAIN} via Vercel!"}
        else:
            return {
                "success": False,
                "message": f"Vercel deployment exited with code {ret}. Set VERCEL_TOKEN or run 'npx vercel login' locally.",
            }

    elif target in ("gh-pages", "github"):
        # GitHub Pages commit / push or workflow trigger
        return {
            "success": True,
            "message": f"Built dist/ for GitHub Pages. Triggered via .github/workflows/deploy-news.yml.",
        }

    return {"success": False, "message": f"Unknown deploy target '{target}'."}


def main():
    parser = argparse.ArgumentParser(description="Telos News CMS & RTC Deployer")
    subparsers = parser.add_subparsers(dest="command")

    subparsers.add_parser("build", help="Compile static news site into dist/")

    serve_parser = subparsers.add_parser("serve", help="Run local RTC news server and editor")
    serve_parser.add_argument("--port", type=int, default=DEFAULT_PORT, help="Port to serve on")

    deploy_parser = subparsers.add_parser("deploy", help="Deploy news to Vercel or GitHub Pages")
    deploy_parser.add_argument("--target", choices=["vercel", "gh-pages"], default="vercel")

    subparsers.add_parser("list", help="List all news articles")

    args = parser.parse_args()

    if args.command == "build" or not args.command:
        build_site()

    elif args.command == "serve":
        build_site()
        port = args.port
        print(f"\n=======================================================")
        print(f" Telos News RTC Studio & Portal")
        print(f" Web Portal:   http://localhost:{port}/")
        print(f" News Studio:  http://localhost:{port}/admin")
        print(f" JSON Feed:    http://localhost:{port}/news.json")
        print(f"=======================================================\n")
        with socketserver.TCPServer(("", port), CMSHandler) as httpd:
            try:
                httpd.serve_forever()
            except KeyboardInterrupt:
                print("\nServer stopped.")

    elif args.command == "deploy":
        res = run_deploy(args.target)
        print(res.get("message"))

    elif args.command == "list":
        articles = load_articles()
        print(f"\nFound {len(articles)} articles:")
        for idx, a in enumerate(articles, 1):
            print(f" {idx}. [{a.get('category')}] {a.get('title')} ({a.get('date')}) - ID: {a.get('id')}")
        print()


if __name__ == "__main__":
    main()
