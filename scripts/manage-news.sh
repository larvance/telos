#!/usr/bin/env bash
# ==============================================================================
# Telos News CMS & Deployment Manager
# Target Domain: news.telos.larvance.com
# ==============================================================================
set -e

SCRIPT_DIR="$(cd -P "$(dirname "${BASH_SOURCE[0]}")" >/dev/null 2>&1 && pwd)"
REPO_DIR="$(cd "$SCRIPT_DIR/.." >/dev/null 2>&1 && pwd)"
NEWS_DIR="$REPO_DIR/web/news"

show_help() {
    echo "Telos News Portal Manager"
    echo ""
    echo "Usage: ./scripts/manage-news.sh [command] [options]"
    echo ""
    echo "Commands:"
    echo "  dev [port]             Start the local RTC News Studio & live preview server (default: 3344)"
    echo "  build                  Build static site into web/news/dist and sync news.json"
    echo "  list                   List all current articles"
    echo "  sync-launcher          Synchronize news feed & artwork directly into apps/telos-launcher"
    echo "  deploy [vercel|pages]  Deploy static news to Vercel or GitHub Pages (news.telos.larvance.com)"
    echo "  help                   Show this help message"
    echo ""
    echo "Examples:"
    echo "  ./scripts/manage-news.sh dev"
    echo "  ./scripts/manage-news.sh build"
    echo "  ./scripts/manage-news.sh deploy vercel"
}

CMD="${1:-dev}"

case "$CMD" in
    dev|serve)
        PORT="${2:-3344}"
        echo "Starting Telos News RTC Studio on http://localhost:$PORT..."
        python3 "$NEWS_DIR/manage.py" serve --port "$PORT"
        ;;
    build)
        echo "Building Telos News portal & feeds..."
        python3 "$NEWS_DIR/manage.py" build
        "$0" sync-launcher
        ;;
    list)
        python3 "$NEWS_DIR/manage.py" list
        ;;
    sync-launcher)
        echo "Synchronizing news feed & artwork to launcher assets..."
        mkdir -p "$REPO_DIR/apps/telos-launcher/ui/assets/news"
        cp "$NEWS_DIR/dist/news.json" "$REPO_DIR/apps/telos-launcher/ui/assets/news/news.json"
        cp -r "$NEWS_DIR/assets/images/"* "$REPO_DIR/apps/telos-launcher/ui/assets/news/" 2>/dev/null || true
        echo "Launcher news assets successfully synchronized."
        ;;
    deploy)
        TARGET="${2:-vercel}"
        echo "Deploying Telos News to $TARGET (Domain: news.telos.larvance.com)..."
        "$0" build
        python3 "$NEWS_DIR/manage.py" deploy --target "$TARGET"
        ;;
    help|--help|-h)
        show_help
        ;;
    *)
        echo "Unknown command: $CMD"
        show_help
        exit 1
        ;;
esac
