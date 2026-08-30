(() => {
  "use strict";

  const ROOT_CLASS = "r2h-embedded";
  const SHELL_ID = "r2h-desktop-shell";
  const BASE_PATH = "/bentopdf/";
  const MARKETING_SELECTORS = [
    "body > nav",
    "body > footer",
    '[data-simple-nav="true"]',
    '[data-simple-footer="true"]',
    "#donation-ribbon",
    "#hero-section",
    "#features-section",
    "#security-compliance-section",
    "#faq-accordion",
    "#testimonials-section",
    "#support-section",
    ".hide-section",
    ".section-divider",
    "#pwa-install",
    "#install-app",
    "[data-pwa-install]",
  ];

  const localizedText = () => {
    const language = (document.documentElement.lang || "").toLowerCase();
    const arabic = language === "ar" || language.startsWith("ar-");

    return arabic
      ? {
          subtitle: "أدوات PDF المحلية",
          allTools: "كل الأدوات",
          license: "ترخيص BentoPDF",
        }
      : {
          subtitle: "Local PDF tools",
          allTools: "All tools",
          license: "BentoPDF license",
        };
  };

  const hideWebsiteChrome = () => {
    for (const selector of MARKETING_SELECTORS) {
      for (const element of document.querySelectorAll(selector)) {
        if (element.id === SHELL_ID) {
          continue;
        }

        element.setAttribute("aria-hidden", "true");
        element.setAttribute("data-r2h-chrome-hidden", "true");
      }
    }
  };

  const hideExternalPromotions = () => {
    for (const anchor of document.querySelectorAll("a[href]")) {
      let url;

      try {
        url = new URL(anchor.href, window.location.href);
      } catch {
        continue;
      }

      if (url.origin === window.location.origin) {
        continue;
      }

      anchor.setAttribute("data-r2h-external-hidden", "true");
      anchor.setAttribute("aria-hidden", "true");
      anchor.tabIndex = -1;
    }
  };

  const makeLink = (href, text) => {
    const anchor = document.createElement("a");
    anchor.className = "r2h-shell__link";
    anchor.href = href;
    anchor.textContent = text;
    return anchor;
  };

  const ensureDesktopShell = () => {
    if (!document.body) {
      return;
    }

    const text = localizedText();
    let shell = document.getElementById(SHELL_ID);

    if (!shell) {
      shell = document.createElement("div");
      shell.id = SHELL_ID;
      shell.setAttribute("role", "banner");
      shell.setAttribute("aria-label", "R2H PDF tools");

      const brand = document.createElement("div");
      brand.className = "r2h-shell__brand";

      const name = document.createElement("strong");
      name.textContent = "R2H PDF";

      const subtitle = document.createElement("span");
      subtitle.className = "r2h-shell__subtitle";
      subtitle.textContent = text.subtitle;

      brand.append(name, subtitle);

      const actions = document.createElement("div");
      actions.className = "r2h-shell__actions";
      actions.append(
        makeLink(`${BASE_PATH}index.html`, text.allTools),
        makeLink(`${BASE_PATH}licensing.html`, text.license),
      );

      const legal = document.createElement("span");
      legal.className = "r2h-shell__license";
      legal.textContent = "BentoPDF 2.8.6 · AGPL-3.0";
      actions.appendChild(legal);

      shell.append(brand, actions);
      document.body.insertBefore(shell, document.body.firstChild);
    }

    const subtitle = shell.querySelector(".r2h-shell__subtitle");
    if (subtitle) {
      subtitle.textContent = text.subtitle;
    }

    const links = shell.querySelectorAll(".r2h-shell__link");
    if (links[0]) {
      links[0].textContent = text.allTools;
    }
    if (links[1]) {
      links[1].textContent = text.license;
    }

    const languageSwitcher = document.getElementById("language-switcher");
    const actions = shell.querySelector(".r2h-shell__actions");

    if (
      languageSwitcher &&
      actions &&
      languageSwitcher.parentElement !== actions
    ) {
      actions.insertBefore(languageSwitcher, actions.firstChild);
    }
  };

  const applyBridge = () => {
    document.documentElement.classList.add(ROOT_CLASS);
    document.documentElement.setAttribute("data-r2h-embedded", "true");

    if (document.body) {
      document.body.classList.add("r2h-embedded-body");
    }

    hideWebsiteChrome();
    hideExternalPromotions();
    ensureDesktopShell();

    if (!document.title.startsWith("R2H PDF")) {
      document.title = `R2H PDF — ${document.title}`;
    }
  };

  let scheduled = false;

  const scheduleApply = () => {
    if (scheduled) {
      return;
    }

    scheduled = true;
    window.requestAnimationFrame(() => {
      scheduled = false;
      applyBridge();
    });
  };

  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", applyBridge, { once: true });
  } else {
    applyBridge();
  }

  const observer = new MutationObserver(scheduleApply);
  observer.observe(document.documentElement, {
    childList: true,
    subtree: true,
  });
})();
