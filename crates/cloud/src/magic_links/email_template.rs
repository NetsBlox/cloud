use lettre::message::MultiPart;
use netsblox_cloud_common::api::MagicLinkId;
use nonempty::NonEmpty;

pub(crate) fn magic_link_email(
    cloud_url: &str,
    usernames: &NonEmpty<String>,
    link_id: &MagicLinkId,
    redirect_uri: Option<String>,
) -> MultiPart {
    let uri_param = redirect_uri
        .map(|uri| format!("&redirectUri={}", uri))
        .unwrap_or_default();

    let make_url = |username| {
        format!(
            "{cloud_url}/magic-links/login?username={username}&linkId={link_id}{uri_param}",
            uri_param = uri_param,
            cloud_url = cloud_url,
            link_id = &link_id.as_str(),
            username = username
        )
    };

    let (txt, html) = if usernames.len() == 1 {
        let url = make_url(usernames.first());
        let html = format!(
            "<h1>Magic sign-in link for NetsBlox</h1>
        <p>
            Please click <a href=\"{url}\">here</a> to \"auto-magically\" sign-in to NetsBlox as {name}.
            <br/>
            <br/>

        Cheers,
        the NetsBlox team</p>",
            name=usernames.first(),
            url=url
        );

        let txt = format!(
            "Magic sign-in link for NetsBlox

        Please click the link below to \"auto-magically\" sign-in to NetsBlox as {name}.

        {url}
            
        Cheers,
        the NetsBlox team",
            url = url,
            name = usernames.first(),
        );

        (txt, html)
    } else {
        let login_links = usernames.iter().fold(String::new(), |prev_text, name| {
            format!(
                "<a href=\"{url}\">{name}</a><br/>{prev_text}",
                name = name,
                url = make_url(name),
                prev_text = prev_text
            )
        });
        let html = format!(
            "<h1>Magic sign-in link for NetsBlox</h1>
        <p>
            Please select an account below to \"auto-magically\" sign-in:
            <br/>
            <br/>

            {login_links}
            
            <br/>
            <br/>
        Cheers,
        the NetsBlox team</p>"
        );

        let url_text = usernames.iter().fold(String::new(), |text, name| {
            format!(
                "{name}: {link}\n{prev_text}",
                name = name,
                link = make_url(name),
                prev_text = text
            )
        });
        let txt = format!(
            "Magic sign-in link for NetsBlox

        Please select an account below to \"auto-magically\" sign-in:

        {urlText}
            
        Cheers,
        the NetsBlox team",
            urlText = url_text,
        );

        (txt, html)
    };

    MultiPart::alternative_plain_html(txt, html)
}
