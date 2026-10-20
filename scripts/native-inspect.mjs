import { chromium } from "@playwright/test";
const browser = await chromium.connectOverCDP("http://127.0.0.1:9225");
const page = browser.contexts().flatMap((context) => context.pages())[0];
page.on("console", (message) => console.log(message.type(), message.text()));
page.on("pageerror", (error) => console.log("pageerror", error.message));
page.on("response", (response) => {
  if (response.status() >= 400)
    console.log("response", response.status(), response.url());
});
await page.reload();
await page.waitForTimeout(1000);
console.log(
  await page.evaluate(() => ({
    url: location.href,
    body: document.body.innerText,
    tauri: !!window.__TAURI_INTERNALS__,
  })),
);
await browser.close();
