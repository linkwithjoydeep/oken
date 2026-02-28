Output assets/demo.gif

Set Shell "zsh"
Set FontSize 13
Set Width 1000
Set Height 500
Set Theme "Catppuccin Mocha"
Set Padding 20
Set TypingSpeed 80ms

# ── setup: seed demo hosts silently ──────────────────────────────────────────
Hide
Type "oken host add prod-web ubuntu@10.0.1.50 --tag prod"
Enter
Sleep 200ms
Type "oken host add prod-db deploy@10.0.1.51 --tag prod --tag db"
Enter
Sleep 200ms
Type "oken host add staging-web ubuntu@10.0.2.10 --tag staging"
Enter
Sleep 200ms
Type "oken host add dev-laptop joy@192.168.1.5 --tag dev"
Enter
Sleep 300ms
Type "clear"
Enter
Sleep 300ms
Show

# ── show host list ────────────────────────────────────────────────────────────
Sleep 500ms
Type "oken host list"
Sleep 300ms
Enter
Sleep 2s

# ── open picker and fuzzy filter ─────────────────────────────────────────────
Type "oken"
Sleep 300ms
Enter
Sleep 1s

Type "prod"
Sleep 2s

Backspace
Backspace
Backspace
Backspace
Sleep 500ms

# ── tag filter ────────────────────────────────────────────────────────────────
Type "#staging"
Sleep 2s

Escape
Sleep 500ms

# ── --tag flag from command line ──────────────────────────────────────────────
Type "oken --tag prod"
Sleep 300ms
Enter
Sleep 1.5s

Escape
Sleep 1s

# ── teardown: remove demo hosts ───────────────────────────────────────────────
Hide
Type "oken host remove prod-web"
Enter
Sleep 200ms
Type "oken host remove prod-db"
Enter
Sleep 200ms
Type "oken host remove staging-web"
Enter
Sleep 200ms
Type "oken host remove dev-laptop"
Enter
Sleep 300ms
Show
