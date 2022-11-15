pub(crate) fn set_password_email(username: &str, url: &str) -> String {
    format!(
        "<h1>Password Reset Request<h1>
        
        Click the link below to reset the password for {username}. If you did not request a password reset, this email can be ignored.
        <br/>
        <br/>
        <a href=\"{url}\">{url}</a>
        <br/>
        <br/>
        Cheers,<br/>
        the NetsBlox team
        ",
        username = username,
        url = url
    )
}
