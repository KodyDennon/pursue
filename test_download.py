from curl_cffi import requests
url = "https://www.war.gov/medialink/ufo/release_1/18_100754_%20general%201946-7_vol_2.pdf"
headers = {
    "Referer": "https://www.war.gov/UFO/",
    "User-Agent": "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/148.0.0.0 Safari/537.36",
}
res = requests.get(url, headers=headers, impersonate="chrome120")
print(f"Status: {res.status_code}")
print(f"Content Length: {len(res.content)}")
if res.status_code == 200:
    with open("test.pdf", "wb") as f:
        f.write(res.content)
