use lettre::message::MultiPart;
use nonempty::NonEmpty;

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

pub(crate) fn forgot_username_email(email: &str, usernames: &NonEmpty<String>) -> MultiPart {
    if usernames.len() > 1 {
        multi_usernames_email(email, usernames)
    } else {
        single_username_email(email, usernames.first())
    }
}

fn multi_usernames_email(email: &str, usernames: &NonEmpty<String>) -> MultiPart {
    MultiPart::alternative_plain_html(
        multi_usernames_txt(email, usernames),
        multi_usernames_html(email, usernames),
    )
}

fn multi_usernames_html(email: &str, usernames: &NonEmpty<String>) -> String {
    let username_list_html = usernames
        .iter()
        .fold(String::new(), |list, name| format!("{}<br/>{}", name, list));

    format!(
        "<h1>NetsBlox Usernames</h1>
        <p>
            This email is just a reminder of the usernames associated with the given email address ({email}).
            <br/>
            <br/>
            {usernameList}
            <br/>
            <br/>
            Cheers,<br/>
            the NetsBlox team
        </p>
        ",
        usernameList = username_list_html,
        email = email
    )
}

fn multi_usernames_txt(email: &str, usernames: &NonEmpty<String>) -> String {
    let username_list_txt = usernames
        .iter()
        .fold(String::new(), |list, name| format!("{}\n{}", name, list));

    format!(
        "NetsBlox Usernames
        
        This email is just a reminder of the usernames associated with the given email address ({email}).


        {usernames}


        Cheers,
        the NetsBlox team",
        usernames = username_list_txt,
        email = email
        )
}

fn single_username_email(email: &str, username: &str) -> MultiPart {
    let html = format!(
        "<h1>NetsBlox Username Reminder</h1>
        <p>
            This email is just a reminder of the username associated with the given email address ({email}): {username}
            <br/>
            <br/>
            Cheers,<br/>
            the NetsBlox team
        </p>
        ",
        username = username,
        email = email
    );

    let txt = format!(
        "NetsBlox Username Reminder
        
        This email is just a reminder of the username associated with the given email address ({email}): {username}

        Cheers,
        the NetsBlox team",
        username = username,
        email = email
        );

    MultiPart::alternative_plain_html(txt, html)
}

#[cfg(test)]
mod tests {
    use super::*;
    use nonempty::nonempty;

    #[actix_web::test]
    async fn test_multi_usernames_html_multiline() {
        let html = multi_usernames_html("EMAIL", &nonempty!["U1".into(), "U2".into()]);
        let on_same_line = html
            .split("<br/>")
            .any(|line| line.contains("U1") && line.contains("U2"));

        assert!(!on_same_line);
    }

    #[actix_web::test]
    async fn test_multi_usernames_txt_multiline() {
        let txt = multi_usernames_txt("EMAIL", &nonempty!["U1".into(), "U2".into()]);
        let on_same_line = txt
            .split("\n")
            .any(|line| line.contains("U1") && line.contains("U2"));

        assert!(!on_same_line);
    }
}
