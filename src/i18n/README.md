# Internationalization (i18n) Documentation

This directory contains the internationalization files for the Notifine Telegram bot system.

## Structure

- `en.json` - English translations (default/fallback language)
- `tr.json` - Turkish translations

## Usage

### Basic Usage with Legacy API

```rust
use notifine::localization::{MESSAGES, Language, detect_user_language};

// Detect user language from Telegram user info
let language = detect_user_language(&user);

// Get a simple message
let welcome_msg = MESSAGES.get("welcome", &language);
```

### New Structured API

```rust
use notifine::localization::{t, t_with_args, Language};

// Simple message
let welcome = t("tesla.welcome", &Language::English);

// Message with arguments
let error_msg = t_with_args(
    "tesla.auth.token_expired", 
    &Language::Turkish, 
    &["Invalid token format"]
);
```

## Message Key Structure

Messages are organized hierarchically using dot notation:

```
tesla.
├── welcome
├── auth.
│   ├── already_authenticated
│   ├── login_required
│   ├── login_url_message
│   ├── success
│   └── ...
├── orders.
│   ├── fetching
│   ├── no_orders_found
│   └── ...
├── monitoring.
│   ├── enabled
│   └── disabled
└── admin.
    ├── only
    └── ...
```

## Adding New Languages

1. Create a new JSON file (e.g., `de.json` for German)
2. Copy the structure from `en.json`
3. Translate all values
4. Add the language to the `Language` enum in `src/localization/mod.rs`
5. Update the `load_translations()` function to include the new language

## Adding New Messages

1. Add the key-value pair to `en.json` (using nested structure)
2. Add the same key with translated value to `tr.json` and other language files
3. Use the message in your code with `t("your.new.key", &language)`

## Argument Substitution

Use `{}` placeholders in your messages for dynamic content:

```json
{
  "tesla": {
    "auth": {
      "token_expired": "Token expired: {}. Please login again."
    }
  }
}
```

Then use with arguments:

```rust
let message = t_with_args("tesla.auth.token_expired", &language, &["Invalid format"]);
```

## Best Practices

1. **Use descriptive keys**: `tesla.auth.login_required` instead of `login_req`
2. **Group related messages**: Keep authentication messages under `tesla.auth.*`
3. **Consistent structure**: Maintain the same key structure across all language files
4. **Fallback to English**: The system automatically falls back to English if a key is missing
5. **Test translations**: Ensure all translations are accurate and culturally appropriate