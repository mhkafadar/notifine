use actix_web::{web, HttpRequest, HttpResponse};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct TeslaAuthCallback {
    pub code: Option<String>,
    pub state: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TeslaAuthPageQuery {
    pub auth_url: Option<String>,
}

pub async fn tesla_auth_callback(
    _req: HttpRequest,
    query: web::Query<TeslaAuthCallback>,
) -> HttpResponse {
    log::info!("TESLA_CALLBACK: Tesla auth callback received: {:?}", query);

    // Create HTML page that will extract the full URL and send it to Telegram
    let html = r#"
<!DOCTYPE html>
<html>
<head>
    <title>Tesla Authentication</title>
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <style>
        body {
            font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
            max-width: 600px;
            margin: 0 auto;
            padding: 20px;
            background-color: #f5f5f5;
            color: #333;
        }
        .container {
            background: white;
            border-radius: 10px;
            padding: 30px;
            box-shadow: 0 2px 10px rgba(0,0,0,0.1);
            text-align: center;
        }
        h1 {
            color: #e82127;
            margin-bottom: 20px;
        }
        .status {
            font-size: 18px;
            margin: 20px 0;
        }
        .success {
            color: #28a745;
        }
        .error {
            color: #dc3545;
        }
        .url-box {
            background: #f8f9fa;
            border: 1px solid #dee2e6;
            border-radius: 5px;
            padding: 15px;
            margin: 20px 0;
            word-break: break-all;
            font-family: monospace;
            font-size: 14px;
            color: #495057;
        }
        .instructions {
            margin: 20px 0;
            color: #6c757d;
        }
        button {
            background-color: #0088cc;
            color: white;
            border: none;
            padding: 12px 24px;
            border-radius: 5px;
            font-size: 16px;
            cursor: pointer;
            margin: 10px;
        }
        button:hover {
            background-color: #0077b5;
        }
        .loading {
            display: inline-block;
            width: 20px;
            height: 20px;
            border: 3px solid #f3f3f3;
            border-top: 3px solid #0088cc;
            border-radius: 50%;
            animation: spin 1s linear infinite;
            margin-left: 10px;
        }
        @keyframes spin {
            0% { transform: rotate(0deg); }
            100% { transform: rotate(360deg); }
        }
    </style>
</head>
<body>
    <div class="container">
        <h1>🚗 Tesla Authentication</h1>
        <div id="status" class="status">Processing authentication...</div>
        <div id="url-container" style="display: none;">
            <div class="url-box" id="url-display"></div>
            <button onclick="copyUrl()">📋 Copy URL</button>
            <button onclick="closeWindow()">✅ Close Window</button>
        </div>
        <div id="instructions" class="instructions" style="display: none;">
            Copy the URL above and send it to the Tesla bot in Telegram.
        </div>
    </div>

    <script src="https://telegram.org/js/telegram-web-app.js"></script>
    <script>
        // Get the current URL
        const currentUrl = window.location.href;
        const urlParams = new URLSearchParams(window.location.search);
        
        // Check if we have an auth code or error
        const authCode = urlParams.get('code');
        const error = urlParams.get('error');
        
        const statusElement = document.getElementById('status');
        const urlContainer = document.getElementById('url-container');
        const urlDisplay = document.getElementById('url-display');
        const instructions = document.getElementById('instructions');
        
        // Initialize Telegram WebApp
        let tg = null;
        if (window.Telegram && window.Telegram.WebApp) {
            tg = window.Telegram.WebApp;
            tg.ready();
            tg.expand();
        }
        
        if (error) {
            statusElement.innerHTML = `<span class="error">❌ Authentication failed: ${error}</span>`;
            statusElement.className = 'status error';
            
            // Send error to WebApp if available
            if (tg && tg.sendData) {
                tg.sendData(JSON.stringify({
                    type: 'tesla_auth_error',
                    error: error,
                    url: currentUrl
                }));
                setTimeout(() => tg.close(), 2000);
            }
            
        } else if (authCode) {
            statusElement.innerHTML = '<span class="success">✅ Authentication successful!</span>';
            statusElement.className = 'status success';
            
            // Check if this is Tesla's void callback
            const isTeslaVoidCallback = currentUrl.includes('auth.tesla.com/void/callback');
            
            if (isTeslaVoidCallback && tg && tg.sendData) {
                console.log('Tesla void callback detected, sending to WebApp:', currentUrl);
                
                // Set main button to show completion
                tg.MainButton.setText("✅ Authentication Complete");
                tg.MainButton.show();
                tg.MainButton.onClick(() => {
                    tg.close();
                });
                
                // Send the callback URL to the bot
                tg.sendData(currentUrl);
                
                // Show success message
                statusElement.innerHTML = '<span class="success">✅ Authentication sent to bot! WebApp will close in 3 seconds...</span>';
                
                // Auto-close after sending
                setTimeout(() => {
                    tg.close();
                }, 3000);
                
            } else if (tg && tg.sendData) {
                // Regular callback handling
                console.log('Regular callback, sending to WebApp:', currentUrl);
                tg.sendData(currentUrl);
                statusElement.innerHTML = '<span class="success">✅ Authentication sent to bot! WebApp will close in 2 seconds...</span>';
                setTimeout(() => tg.close(), 2000);
                
            } else {
                console.log('Telegram WebApp not available, showing manual copy option');
                // Display the URL for manual copying
                urlDisplay.textContent = currentUrl;
                urlContainer.style.display = 'block';
                instructions.style.display = 'block';
            }
            
        } else {
            statusElement.innerHTML = '<span class="error">❌ No authentication code received</span>';
            statusElement.className = 'status error';
        }
        
        function copyUrl() {
            navigator.clipboard.writeText(currentUrl).then(() => {
                const button = event.target;
                const originalText = button.textContent;
                button.textContent = '✅ Copied!';
                setTimeout(() => {
                    button.textContent = originalText;
                }, 2000);
            }).catch(err => {
                console.error('Failed to copy:', err);
                alert('Failed to copy URL. Please select and copy manually.');
            });
        }
        
        function closeWindow() {
            if (window.Telegram && window.Telegram.WebApp) {
                window.Telegram.WebApp.close();
            } else {
                window.close();
            }
        }
    </script>
</body>
</html>
"#;

    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(html)
}

pub async fn tesla_success_page(
    _req: HttpRequest,
    query: web::Query<TeslaAuthCallback>,
) -> HttpResponse {
    log::info!("TESLA_SUCCESS: Tesla success page accessed: {:?}", query);

    // This is the generic success page that users will see after Tesla redirects them
    let html = r#"
<!DOCTYPE html>
<html>
<head>
    <title>Tesla Authentication Success</title>
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <style>
        body {
            font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
            max-width: 600px;
            margin: 0 auto;
            padding: 20px;
            background: var(--tg-theme-bg-color, #f5f5f5);
            color: var(--tg-theme-text-color, #333);
        }
        .container {
            background: var(--tg-theme-secondary-bg-color, white);
            border-radius: 10px;
            padding: 30px;
            box-shadow: 0 2px 10px rgba(0,0,0,0.1);
            text-align: center;
        }
        h1 {
            color: #e82127;
            margin-bottom: 20px;
        }
        .success {
            background-color: #d4edda;
            color: #155724;
            border: 1px solid #c3e6cb;
            padding: 15px;
            border-radius: 5px;
            margin: 20px 0;
        }
        .url-box {
            background: #f8f9fa;
            border: 1px solid #dee2e6;
            border-radius: 5px;
            padding: 15px;
            margin: 20px 0;
            word-break: break-all;
            font-family: monospace;
            font-size: 14px;
            color: #495057;
        }
        button {
            background-color: var(--tg-theme-button-color, #e82127);
            color: var(--tg-theme-button-text-color, white);
            border: none;
            padding: 12px 24px;
            border-radius: 5px;
            font-size: 16px;
            cursor: pointer;
            margin: 10px;
        }
        button:hover {
            opacity: 0.9;
        }
        .instructions {
            margin: 20px 0;
            color: var(--tg-theme-hint-color, #6c757d);
            line-height: 1.5;
        }
    </style>
    <script src="https://telegram.org/js/telegram-web-app.js"></script>
</head>
<body>
    <div class="container">
        <h1>🚗 Tesla Authentication</h1>
        <div class="success">✅ Authentication successful!</div>
        
        <div class="instructions">
            Your Tesla authentication was successful. The authentication data has been automatically sent to the bot.
        </div>
        
        <div class="url-box" id="url-display"></div>
        <button onclick="copyUrl()">📋 Copy URL</button>
        <button onclick="closeWindow()">✅ Close</button>
    </div>

    <script>
        const currentUrl = window.location.href;
        const urlParams = new URLSearchParams(window.location.search);
        const authCode = urlParams.get('code');
        const error = urlParams.get('error');
        
        document.getElementById('url-display').textContent = currentUrl;
        
        // Initialize Telegram WebApp
        let tg = null;
        if (window.Telegram && window.Telegram.WebApp) {
            tg = window.Telegram.WebApp;
            tg.ready();
            tg.expand();
            
            // Automatically send data to bot if this is a WebApp
            if (authCode && !error) {
                console.log('Sending Tesla success data to WebApp:', currentUrl);
                tg.sendData(currentUrl);
                
                // Show close button
                tg.MainButton.setText("✅ Close");
                tg.MainButton.show();
                tg.MainButton.onClick(() => {
                    tg.close();
                });
                
                // Auto-close after 3 seconds
                setTimeout(() => {
                    tg.close();
                }, 3000);
            }
        }
        
        function copyUrl() {
            navigator.clipboard.writeText(currentUrl).then(() => {
                const button = event.target;
                const originalText = button.textContent;
                button.textContent = '✅ Copied!';
                setTimeout(() => {
                    button.textContent = originalText;
                }, 2000);
            }).catch(err => {
                console.error('Failed to copy:', err);
                alert('Failed to copy URL. Please select and copy manually.');
            });
        }
        
        function closeWindow() {
            if (tg) {
                tg.close();
            } else {
                window.close();
            }
        }
    </script>
</body>
</html>
"#;

    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(html)
}

pub async fn tesla_auth_page(
    _req: HttpRequest,
    query: web::Query<TeslaAuthPageQuery>,
) -> HttpResponse {
    let auth_url = query.auth_url.clone().unwrap_or_default();
    log::info!("TESLA_AUTH_PAGE: Serving auth page with URL: {}", auth_url);

    // This page initiates the Tesla OAuth flow and handles the callback within the WebApp
    let html = format!(
        r#"
<!DOCTYPE html>
<html>
<head>
    <title>Tesla Login</title>
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <style>
        body {{
            font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
            max-width: 600px;
            margin: 0 auto;
            padding: 20px;
            background: var(--tg-theme-bg-color, #f5f5f5);
            color: var(--tg-theme-text-color, #333);
        }}
        .container {{
            background: var(--tg-theme-secondary-bg-color, white);
            border-radius: 10px;
            padding: 30px;
            box-shadow: 0 2px 10px rgba(0,0,0,0.1);
            text-align: center;
        }}
        h1 {{
            color: #e82127;
            margin-bottom: 20px;
        }}
        button {{
            background-color: var(--tg-theme-button-color, #e82127);
            color: var(--tg-theme-button-text-color, white);
            border: none;
            padding: 15px 30px;
            border-radius: 5px;
            font-size: 18px;
            cursor: pointer;
            margin: 20px 0;
            width: 100%;
            max-width: 300px;
        }}
        button:hover {{
            opacity: 0.9;
        }}
        button:disabled {{
            opacity: 0.5;
            cursor: not-allowed;
        }}
        .info {{
            color: var(--tg-theme-hint-color, #6c757d);
            margin: 20px 0;
            line-height: 1.5;
        }}
        .status {{
            margin: 20px 0;
            padding: 15px;
            border-radius: 5px;
            font-weight: bold;
        }}
        .success {{
            background-color: #d4edda;
            color: #155724;
            border: 1px solid #c3e6cb;
        }}
        .error {{
            background-color: #f8d7da;
            color: #721c24;
            border: 1px solid #f5c6cb;
        }}
        .loading {{
            display: inline-block;
            width: 20px;
            height: 20px;
            border: 3px solid #f3f3f3;
            border-top: 3px solid var(--tg-theme-button-color, #e82127);
            border-radius: 50%;
            animation: spin 1s linear infinite;
            margin-left: 10px;
        }}
        @keyframes spin {{
            0% {{ transform: rotate(0deg); }}
            100% {{ transform: rotate(360deg); }}
        }}
    </style>
    <script src="https://telegram.org/js/telegram-web-app.js"></script>
</head>
<body>
    <div class="container">
        <h1>🚗 Tesla Account Login</h1>
        <div id="status" class="info">
            Click the button below to securely login to your Tesla account within Telegram.
        </div>
        <button id="login-btn" onclick="startAuth()">Login with Tesla</button>
    </div>

    <script>
        const authUrl = '{auth_url}';
        let tg = window.Telegram.WebApp;
        
        // Initialize Telegram Web App
        if (tg) {{
            tg.ready();
            tg.expand();
            
            // Set main button
            tg.MainButton.setText("Cancel");
            tg.MainButton.onClick(() => {{
                tg.close();
            }});
            tg.MainButton.show();
        }}
        
        function showStatus(message, isError = false) {{
            const statusEl = document.getElementById('status');
            statusEl.textContent = message;
            statusEl.className = isError ? 'status error' : 'status success';
        }}
        
        function startAuth() {{
            if (!authUrl) {{
                showStatus('Authentication URL not provided. Please restart the login process.', true);
                return;
            }}
            
            const loginBtn = document.getElementById('login-btn');
            
            loginBtn.disabled = true;
            loginBtn.innerHTML = 'Redirecting to Tesla<span class="loading"></span>';
            
            showStatus('Redirecting to Tesla login page...');
            
            // Show instructions before redirect
            if (tg && tg.showAlert) {{
                tg.showAlert("After login, you'll see a 'Page Not Found' error. This is normal! Copy the complete URL from your browser's address bar and return to this chat.");
            }}
            
            // Redirect directly - Tesla will redirect back to void callback
            window.location.href = authUrl;
        }}
        
    </script>
</body>
</html>
"#,
        auth_url = auth_url
    );

    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(html)
}
