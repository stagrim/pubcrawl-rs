use maud::{Markup, html};

use crate::routes::utils::theme_extractor::{THEME_COOKIE_NAME, Theme};

pub fn base(theme: Theme, content: Markup) -> Markup {
    html! {
        (maud::DOCTYPE)
        html data-theme=(theme) {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1";
                title { "Pubcrawl" }
                link rel="icon" href="/static/icons/favicon.png";
                link rel="stylesheet" href="/static/css/style.min.css?v=1";
                script src="/static/js/htmx.min.js?v=4.0.1" defer {}
                script src="/static/js/hx-see.min.js?v=4.0.1" defer {}
                script src="/static/js/script.js?v=1" defer {}
            }
            body class="theme-controller" {
                nav class="navbar dark:bg-base-300 bg-blue-800 text-white shadow-lg" {
                    div class="navbar-start" {
                        a href="/" {
                            img src="/static/icons/logo-white.webp" alt="Pubcrawl logo" width="54" {}
                        }
                        a class="btn btn-ghost text-2xl font-bold" href="/" { "Pubcrawl" }
                    }
                    div class="navbar-middle hidden lg:inline-flex" {
                        a class="btn btn-ghost" href="/about" { "About" }
                    }
                    div class="navbar-end" {
                        a class="btn btn-ghost hidden lg:inline-flex" href="/count" { "Count" }
                        (light_switch(theme))
                    }
                    div class="dropdown dropdown-end" {
                        div tabindex="0" role="button" class="btn btn-ghost lg:hidden" {
                            svg xmlns="http://www.w3.org/2000/svg" width="32" height="32" viewBox="0 0 24 24" {
                                path fill="currentColor" d="M4 18q-.425 0-.712-.288T3 17t.288-.712T4 16h16q.425 0 .713.288T21 17t-.288.713T20 18zm0-5q-.425 0-.712-.288T3 12t.288-.712T4 11h16q.425 0 .713.288T21 12t-.288.713T20 13zm0-5q-.425 0-.712-.288T3 7t.288-.712T4 6h16q.425 0 .713.288T21 7t-.288.713T20 8z" {}
                            }
                        }
                        ul tabindex="0" class="menu menu-sm dropdown-content rounded-box dark:bg-base-300 z-1 mt-3 w-52 bg-blue-800 p-2 text-white shadow" {
                            li { a class="btn btn-outline m-1" href="/about" { "About" } }
                            li { a class="btn btn-outline m-1" href="/count" { "Count" } }
                        }
                    }
                }

                div class="m-0 flex min-h-screen w-screen flex-col p-0" {
                    main class="mx-auto mt-3 flex w-11/12 flex-grow flex-col text-center sm:w-10/12 sm:text-left lg:w-8/12" {
                        (content)
                    }
                }
            }
        }
    }
}

fn light_switch(theme: Theme) -> Markup {
    html! {
        label class="swap swap-rotate" {
            input
                type="checkbox"
                class="theme-controller"
                checked[(match theme {
                    Theme::Dark => true,
                    _ => false,
                })]
                hx-on:change={"
                    const theme = this.checked ? 'dark' : 'light';
                    document.documentElement.setAttribute('data-theme', theme);
                    document.cookie = `"(THEME_COOKIE_NAME)"=${theme}; path=/; max-age=31536000; SameSite=Lax; Secure`;
                "};

            svg
                class="swap-off h-10 w-10 fill-current"
                xmlns="http://www.w3.org/2000/svg"
                viewBox="0 0 24 24"
            {
                path d="M5.64,17l-.71.71a1,1,0,0,0,0,1.41,1,1,0,0,0,1.41,0l.71-.71A1,1,0,0,0,5.64,17ZM5,12a1,1,0,0,0-1-1H3a1,1,0,0,0,0,2H4A1,1,0,0,0,5,12Zm7-7a1,1,0,0,0,1-1V3a1,1,0,0,0-2,0V4A1,1,0,0,0,12,5ZM5.64,7.05a1,1,0,0,0,.7.29,1,1,0,0,0,.71-.29,1,1,0,0,0,0-1.41l-.71-.71A1,1,0,0,0,4.93,6.34Zm12,.29a1,1,0,0,0,.7-.29l.71-.71a1,1,0,1,0-1.41-1.41L17,5.64a1,1,0,0,0,0,1.41A1,1,0,0,0,17.66,7.34ZM21,11H20a1,1,0,0,0,0,2h1a1,1,0,0,0,0-2Zm-9,8a1,1,0,0,0-1,1v1a1,1,0,0,0,2,0V20A1,1,0,0,0,12,19ZM18.36,17A1,1,0,0,0,17,18.36l.71.71a1,1,0,0,0,1.41,0,1,1,0,0,0,0-1.41ZM12,6.5A5.5,5.5,0,1,0,17.5,12,5.51,5.51,0,0,0,12,6.5Zm0,9A3.5,3.5,0,1,1,15.5,12,3.5,3.5,0,0,1,12,15.5Z";
            }

            svg
                class="swap-on h-10 w-10 fill-current"
                xmlns="http://www.w3.org/2000/svg"
                viewBox="0 0 24 24"
            {

                path d="M21.64,13a1,1,0,0,0-1.05-.14,8.05,8.05,0,0,1-3.37.73A8.15,8.15,0,0,1,9.08,5.49a8.59,8.59,0,0,1,.25-2A1,1,0,0,0,8,2.36,10.14,10.14,0,1,0,22,14.05,1,1,0,0,0,21.64,13Zm-9.5,6.69A8.14,8.14,0,0,1,7.08,5.22v.27A10.15,10.15,0,0,0,17.22,15.63a9.79,9.79,0,0,0,2.1-.22A8.11,8.11,0,0,1,12.14,19.73Z";
            }
        }
    }
}
