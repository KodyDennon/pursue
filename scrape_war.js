const puppeteer = require('puppeteer');

(async () => {
  const browser = await puppeteer.launch();
  const page = await browser.newPage();
  await page.goto('https://www.war.gov/UFO/', { waitUntil: 'networkidle2' });
  const html = await page.content();
  require('fs').writeFileSync('war_gov_ufo.html', html);
  await browser.close();
  console.log("Done");
})();
