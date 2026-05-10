from curl_cffi import requests
res = requests.get('https://www.war.gov/UFO/', impersonate='chrome120')
with open('war_gov.html', 'w') as f:
    f.write(res.text)
print("Saved html, len:", len(res.text))
