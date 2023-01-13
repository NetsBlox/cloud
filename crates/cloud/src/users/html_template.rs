pub(crate) fn set_password_page(username: &str) -> String {
    format!("
<html>
    <head>
        <link rel=\"stylesheet\" href=\"https://fonts.googleapis.com/icon?family=Material+Icons\"/>
        <link rel=\"stylesheet\" href=\"https://cdnjs.cloudflare.com/ajax/libs/materialize/0.98.2/css/materialize.min.css\"/>
        <meta name=\"viewport\" content=\"width=device-width, initial-scale=1.0\"/>
        <title>Set NetsBlox Password</title>
    </head>
    <body>
        <div style=\"height: 20%\"></div>
        <div class=\"row\">
            <div class=\"col s6 offset-s3\">
                <p class=\"flow-text center-align\">Enter the new password for {username}:</p>
            </div>
        </div>
        <div class=\"row\">
            <div class=\"center-align\">
                <form id=\"setPasswordForm\" method=\"post\">
                    <label for=\"pwd\"></label><br/>
                    <input type=\"password\" id=\"pwd\" required>
                    <label for=\"confirmPwd\"></label><br/>
                    <input type=\"password\" id=\"confirmPwd\" required>
                </form>
                <a id=\"setPasswordButton\" class=\"waves-effect waves-light btn\">Change Password</a>
                <p id=\"errorMsg\" class=\"errorMsg\"></p>
                <p id=\"successMsg\" class=\"successMsg\"></p>
            </div>
        </div>

        <script src=\"https://cdnjs.cloudflare.com/ajax/libs/materialize/1.0.0-beta/js/materialize.min.js\"></script>
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
                if (response.status === 200) {{
                    successMsg.innerHTML = 'Password updated!';
                }} else {{
                    errorMsg.innerHTML = 'Unable to set password: ' + await response.text();
                }}
                console.log(response.status);
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
