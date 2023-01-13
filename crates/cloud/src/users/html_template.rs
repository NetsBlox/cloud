pub(crate) fn set_password_page(username: &str) -> String {
    format!("
<html>
    <head>
        <link rel=\"stylesheet\" href=\"https://fonts.googleapis.com/icon?family=Material+Icons\"/>
        <link rel=\"stylesheet\" href=\"https://cdnjs.cloudflare.com/ajax/libs/materialize/1.0.0/css/materialize.min.css\"/>
        <meta name=\"viewport\" content=\"width=device-width, initial-scale=1.0\"/>
        <title>Set NetsBlox Password</title>
    </head>
    <body>

        <div class=\"blue-grey lighten-5 valign-wrapper\" style=\"height:100%\">
            <div class=\"container\">
                <div class=\"row\">
                    <div class=\"col s12 m8 offset-m2 l6 offset-l3\">
                        <div class=\"card\">
                            <div class=\"card-content\">
                                <span class=\"card-title\">Enter a new password for {username}:</span>

                                <div class=\"row\">
                                    <label for=\"pwd\"></label><br/>
                                    <input type=\"password\" id=\"pwd\" required>
                                    <label for=\"confirmPwd\"></label><br/>
                                    <input type=\"password\" id=\"confirmPwd\" required>
                                </div>
                                <div class=\"row\">
                                    <a id=\"setPasswordButton\" class=\"waves-effect waves-light btn\">Change Password</a>
                                    <p id=\"errorMsg\" class=\"errorMsg\"></p>
                                    <p id=\"successMsg\" class=\"successMsg\"></p>
                                </div>
                            </div>
                        </div>
                    </div>
                </div>
            </div>
        </div>

        <script src=\"https://cdnjs.cloudflare.com/ajax/libs/materialize/1.0.0/js/materialize.min.js\"></script>
        <script>
            const queryString = location.href.split('?').pop();
            const allowButton = document.getElementById('setPasswordButton');
            const errorMsg = document.getElementById('errorMsg');
            const successMsg = document.getElementById('successMsg');
            allowButton.onclick = async function() {{
                // ensure the passwords match
                const password = document.getElementById('pwd').value;
                const passwordConfirm = document.getElementById('confirmPwd');
                if (password !== passwordConfirm.value) {{
                    return passwordConfirm.setCustomValidity('Passwords do not match');
                }}
                passwordConfirm.setCustomValidity('');
                errorMsg.innerHTML = '';

                const url = location.href;
                const opts = {{
                    method: 'PATCH',
                    headers: {{
                        'Content-Type': 'application/json',
                    }},
                    body: JSON.stringify(password)
                }};
                const response = await fetch(url, opts);
                const hasToken = url.includes('token');
                if (response.status === 200) {{
                    successMsg.innerHTML = 'Password updated!';
                }} else if (response.status === 403 && hasToken) {{
                    errorMsg.innerHTML = 'Unable to set password: Invalid reset token.';
                }} else {{
                    errorMsg.innerHTML = 'Unable to set password: ' + await response.text();
                }}
            }};
        </script>
        <style>
            .successMsg {{
                color: blue;
            }}

            .errorMsg {{
                color: red;
            }}

            .username {{
                font-size: 1.5em;
                text-align: center;
            }}

            @media only screen and (min-width: 750px){{
                .scope {{
                    font-size: 1.25em;
                }}
            }}
        </style>
<html/>
    ", username=username)
}
