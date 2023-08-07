use super::routes::Scope;

pub(crate) fn authorize_page(username: &str, client: &str, scopes: &[Scope]) -> String {
    let scope_html: String = scopes
        .iter()
        .map(|name| format!("<tr><td class=\"scope\">{}</td></tr>", name))
        .collect();

    // TODO: fix the image
    format!("
<html>
    <head>
        <link rel=\"stylesheet\" href=\"https://fonts.googleapis.com/icon?family=Material+Icons\"/>
        <link rel=\"stylesheet\" href=\"https://cdnjs.cloudflare.com/ajax/libs/materialize/0.98.2/css/materialize.min.css\"/>
        <meta name=\"viewport\" content=\"width=device-width, initial-scale=1.0\"/>
        <title>Allow NetsBlox Access?</title>
    </head>
    <body>
        <img style=\"display: block; margin: auto;\" class=\"img logo img-responsive\" height=\"60px\" src=\"/oauth/netsblox_logo.png\" alt=\"NetsBlox\"></img>

        <div style=\"height: 20%\"></div>
        <div class=\"row\">
            <div class=\"col s6 offset-s3\">
                <p class=\"flow-text center-align\"><span class=\"client\">{client}</span> wants to access your NetsBlox account</p>
                <p class=\"username\">{username}</p>
                <p class=\"flow-text center-align\">This will allow <span class=\"client\">{client}</span> to:</p>
                <table>
                {scopes}
                </table>
            </div>
        </div>
        <div class=\"row\">
            <div class=\"center-align\">
                <form id=\"allow\" action=\"/oauth/{username}/code\" method=\"post\"></form>
                <a id=\"denyButton\" class=\"waves-effect waves-light btn grey\">Deny</a>
                <a id=\"allowButton\" class=\"waves-effect waves-light btn\">Allow</a>
            </div>
        </div>

        <script src=\"https://cdnjs.cloudflare.com/ajax/libs/materialize/1.0.0-beta/js/materialize.min.js\"></script>
        <script>
            const queryString = location.href.split('?').pop();
            const allowButton = document.getElementById('allowButton');
            allowButton.onclick = async function() {{
                const allowForm = document.getElementById('allow');
                allowForm.setAttribute('action', allowForm.getAttribute('action') + '?' + queryString);
                allowForm.submit();
            }};

            const denyButton = document.getElementById('denyButton');
            denyButton.onclick = function() {{
                const allowForm = document.getElementById('allow');
                const url = allowForm.getAttribute('action') + '?' + queryString +
                    '&error=' + encodeURIComponent('access_denied') + '&error_description=' +
                    encodeURIComponent('The user denied the request.');
                allowForm.setAttribute('action', url);
                allowForm.submit();
            }};
        </script>
        <style>
            .client {{
                color: blue;
            }}

            .username {{
                font-size: 1.5em;
                text-align: center;
            }}

            .scope {{
                font-size: 1em;
                font-style: italic;
                text-align: center;
                color: #37474f;
            }}
            @media only screen and (min-width: 750px){{
                .scope {{
                    font-size: 1.25em;
                }}
            }}
        </style>
<html/>
    ", username=username, client=client, scopes=scope_html)
}
