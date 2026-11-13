
## Deezer debug cookie probe

In a development build, open Settings → Advanced → Debug and choose “Run Deezer cookie probe”. Complete Deezer login in the isolated diagnostic webview. The result is an in-app evidence report listing cookie names and HttpOnly, Secure, and SameSite flags, plus the HttpOnly status of `arl`; cookie values are never included. Closing the window early produces no report. The probe command is compiled only with Rust debug assertions and does not participate in account authentication.
