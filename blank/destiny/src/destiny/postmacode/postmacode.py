import requests


class PostMaCode:
    def __init__(self):
        self.base_url = "https://api.postcodes.io/postcodes/"

    def clean_postcode(self, postcode: str) -> str:
        cleaned_postcode = (
            postcode.strip().strip(".").upper().replace("  ", " ").replace("   ", " ")
        )

        # If there is no space before last 3 characters, insert one
        if len(cleaned_postcode) >= 4 and cleaned_postcode[-4] != " ":
            cleaned_postcode = cleaned_postcode[:-3] + " " + cleaned_postcode[-3:]

        return cleaned_postcode

    def postcode_to_geo(self, postcode: str) -> dict:

        postcode = self.clean_postcode(postcode)

        response = requests.get(f"{self.base_url}{postcode}")
        if response.status_code == 200:
            data = response.json()
            return {
                "postcode": postcode,
                "latitude": data["result"]["latitude"],
                "longitude": data["result"]["longitude"],
            }
        else:
            return {"error": "Invalid postcode"}

    def batch_postcode_to_geo(self, postcodes: list) -> list:
        results = []
        for postcode in postcodes:
            result = self.postcode_to_geo(postcode)
            results.append(result)
        return results
