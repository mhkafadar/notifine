<div align="center">

<a href="https://notifine.com" align="center"><h1>notifine</h1></a>

<p align="center">
Bots for Telegram. Get notifications on new events like new commits, pipelines, etc.
</p>

</div>

Notifine helps with events in your work tools like Gitlab, Github, and Trello. Just send a message to notifine or add it to your telegram group. It will provide you with a webhook link right away. Copy this link and paste it to your favorite tool (only GitLab now).

- 🤖 [Gitlab bot](https://t.me/gitlab_notifine_bot)
- 🤖 [Github bot](https://t.me/github_notifine_bot)
- 🧭 Trello bot (Soon)
- 🚗 Tesla bot - Track your Tesla order status
- 🔔 [Beep bot](https://t.me/beep_notifine_bot) - Simple webhook notifications
- 📊 [Uptime bot](https://t.me/uptime_notifine_bot) - Monitor website availability

---

**Mar 18, 2023: Github bot has started working!**

**Dec 04, 2022: Gitlab bot has started working!**

---

## Available Notifications

### Github and GitLab

✅ Commit push  
 ✅ Tag push  
 ✅ Pipeline  
 ✅ Pull request (via Job events)  
 ✅ Job events  
 ✅ Issue  
 ✅ Comment  
 ✅ Deployment notifications

### Tesla Bot

✅ OAuth authentication with Tesla account  
✅ Real-time order status tracking  
✅ Automatic status change notifications (checks every 5 minutes)  
✅ Detailed order information (VIN, delivery dates, etc.)  
✅ Secure token storage with AES-256 encryption  
✅ Smart change detection - only notifies when something changes

## ✨ Features

**😎 100% open source and free**

Backend (Rust) and infrastructure codes (AWS CDK) are open sources.

**🚀 Uptime guarantee**

There are some telegram bots available already. But they are not able to provide a continuous experience because of long downtimes. This is the main reason for the existence of notifine.

Check the uptime status of the bots: https://stats.uptimerobot.com/5zQqxuMGNY

<img src="./assets/uptime.png" alt="uptime robot screenshot" width="600px"/>

**🎓 Educational purposes**

I am planning to prepare a tutorial based on this project. And help new Rustaceans with an up-to-date Rust implementation.

**⚡️ Built on Rust**

Benefit Rust features of speed, type safety, async.

**🔌 On premise available**

If you want to host the bot on your private server.

## Notes

Gitlab sends a job event for each PR event. So, to avoid duplicate notifications, we use job events instead of PR events. If you want to test PR events, you should select Job event and click test on Gitlab

**Telegram threads (topics) are now supported:**
After adding the bot to the group, simply execute the /start command in the thread where you want to receive notifications.

## Branch Filtering

Both GitHub and GitLab webhooks support branch filtering to reduce notification noise from development branches. Add query parameters to your webhook URL to filter events by branch patterns.

### Usage Examples

```bash
# Only receive notifications for main branch
https://webhook.notifine.com/github/webhook123?branch=main
https://webhook.notifine.com/gitlab/webhook123?branch=main

# Multiple branches
https://webhook.notifine.com/github/webhook123?branch=main,develop

# Wildcard patterns
https://webhook.notifine.com/github/webhook123?branch=release/*

# Exclude development branches
https://webhook.notifine.com/github/webhook123?exclude_branch=feature/*,dependabot/*

# Combine include and exclude (exclude takes precedence)
https://webhook.notifine.com/github/webhook123?branch=main,release/*&exclude_branch=*-wip,*-temp
```

### Supported Events

**GitHub:** Push events, Pull Request events, Workflow Run events, Create/Delete events
**GitLab:** Push events, Merge Request events

### Filter Rules

- **Include patterns** (`branch=`): Only process events for matching branches
- **Exclude patterns** (`exclude_branch=`): Never process events for matching branches
- **Precedence**: Exclusions take priority over inclusions
- **Wildcards**: Use `*` for pattern matching (e.g., `feature/*`, `*-wip`)
- **Multiple patterns**: Separate with commas (`main,develop,release/*`)

### Backward Compatibility

- Webhooks without branch filters continue to receive all events
- GitLab's existing `?full_message=true` parameter still works alongside branch filtering

## Configuration

### Environment Variables

The following environment variables are required for running Notifine:

```bash
# Database
DATABASE_URL=postgresql://user:password@localhost/notifine

# Webhook base URL
WEBHOOK_BASE_URL=https://webhook.notifine.com

# Telegram Bot Tokens
GITLAB_TELOXIDE_TOKEN=your_gitlab_bot_token
GITHUB_TELOXIDE_TOKEN=your_github_bot_token
BEEP_TELOXIDE_TOKEN=your_beep_bot_token
TRELLO_TELOXIDE_TOKEN=your_trello_bot_token
UPTIME_TELOXIDE_TOKEN=your_uptime_bot_token
TESLA_TELOXIDE_TOKEN=your_tesla_bot_token

# Security - Token Encryption
TESLA_ENCRYPTION_KEY=  # 64 hex characters for AES-256 encryption

# Admin Configuration
ADMIN_LOGS=NOT_ACTIVE  # ACTIVE or NOT_ACTIVE
ADMIN_LOG_LEVEL=0
TELEGRAM_ADMIN_CHAT_ID=your_admin_chat_id
```

### Security

#### Token Encryption

Notifine uses AES-256-GCM encryption to protect sensitive OAuth tokens stored in the database. This ensures that even if the database is compromised, tokens remain secure.

**Generating an Encryption Key:**

```bash
# Generate a secure 256-bit key (64 hex characters)
openssl rand -hex 32
```

**Important Security Notes:**

- Store the encryption key securely (use environment variables, never commit to git)
- Use different encryption keys for each environment (dev, staging, production)
- Rotate encryption keys periodically
- Back up your encryption keys securely - lost keys mean inaccessible tokens

### Database Setup

Run migrations to set up the database schema:

```bash
# Install diesel CLI
cargo install diesel_cli --no-default-features --features postgres

# Run migrations
diesel migration run

# Or using Docker Compose
docker-compose up -d
```

## Ways to help

Regardless of your experience level, there are ways that you can help:

- Share the project on Twitter
- Star the project on GitHub
- Tell your friends/co-workers about Notifine
- Write an article about Notifine on Medium, Dev, or your platform of choice
- Report bugs or provide feedback by [creating issues](https://github.com/mhkafadar/gitlab-telegram/issues)
- Contribute to the source code by fixing bugs/issues or helping us build new features
