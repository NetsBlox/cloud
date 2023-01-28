use lettre::message::MultiPart;

pub(crate) fn set_password_email(username: &str, url: &str) -> MultiPart {
    let html =    format!(
        "<h1>Password Reset Request</h1>
        <p>
            Click the link below to reset the password for {username}. If you did not request a password reset, this email can be ignored.
            <br/>
            <br/>
            <a href=\"{url}\">{url}</a>
            <br/>
            <br/>
            Cheers,<br/>
            the NetsBlox team
        </p>
        ",
        username = username,
        url = url
    );
    let txt = format!(
        "Password Reset Request
        
        Click the link below to reset the password for {username}. If you did not request a password reset, this email can be ignored.


        {url}


        Cheers,
        the NetsBlox team",
        username = username,
        url = url
        );

    MultiPart::alternative_plain_html(txt, html)
}
