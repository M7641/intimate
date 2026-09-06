import requests
import json
import urllib.parse
import haversine


def find_routes_between_points(
    start: tuple,
    end: tuple,
):

    # Prepare the query
    query = f"""
    [bbox:{start[0]},{start[1]},{end[0]},{end[1]}]
    [out:json]
    [timeout:90]
    ;
    (
    way
    (
        {start[0]},
        {start[1]},
        {end[0]},
        {end[1]}
    )
    [highway~"^(motorway|trunk|primary|secondary|tertiary|unclassified|residential|motorway_link|trunk_link|primary_link|secondary_link|tertiary_link)$"]
    [!"building"]
    [!"leisure"]
    [!"service"]
    [!"waterway"]
    [!"amenity"]
    [!"barrier"]
    [!"power"]
    ["access"!="private"]
    ["access"!="no"];
    );
    (._;>;);
    out geom;
    """

    # Make the POST request
    response = requests.post(
        "https://overpass-api.de/api/interpreter",
        data="data=" + urllib.parse.quote(query),
    )

    return response.json()


def convert_overpass_to_locations(overpass_data: dict) -> list:

    formatted_locations = []
    for element in overpass_data.get("elements", []):
        if element["type"] == "way":
            for node in element.get("geometry", []):
                formatted_locations.append({"lat": node["lat"], "lng": node["lon"]})

    # Remove locations that are too close to each other (within 0.5 km)
    filtered_locations = []
    if formatted_locations:
        filtered_locations.append(formatted_locations[0])

        for location in formatted_locations[1:]:
            too_close = False
            for filtered_loc in filtered_locations:
                distance = haversine.haversine(
                    (location["lat"], location["lng"]),
                    (filtered_loc["lat"], filtered_loc["lng"]),
                )
                if distance < 0.5:
                    too_close = True
                    break

                location["distance"] = distance
                # Assuming average speed of 30 km/h
                location["travelTimes"] = distance / 30 * 60

            if not too_close:
                filtered_locations.append(location)

        formatted_locations = filtered_locations

    # Write to JSON file
    with open("locations.json", "w") as f:
        json.dump(formatted_locations, f, indent=2)

    return formatted_locations


def create_travel_matrix(locations: list) -> dict:

    num_locations = len(locations)
    distances = [[0 for _ in range(num_locations)] for _ in range(num_locations)]
    travel_times = [[0 for _ in range(num_locations)] for _ in range(num_locations)]

    for i in range(num_locations):
        for j in range(num_locations):
            if i != j:
                lat1, lon1 = locations[i]["lat"], locations[i]["lng"]
                lat2, lon2 = locations[j]["lat"], locations[j]["lng"]

                distance = haversine.haversine((lat1, lon1), (lat2, lon2))
                distances[i][j] = int(distance * 1000)  # Convert to meters
                travel_times[i][j] = int(
                    distance / 30 * 60
                )  # Assuming average speed of 30 km/h
            else:
                distances[i][j] = 0
                travel_times[i][j] = 0

    # Flatten the 2D matrices into 1D arrays
    flat_distances = []
    flat_travel_times = []
    for i in range(num_locations):
        for j in range(num_locations):
            flat_distances.append(distances[i][j])
            flat_travel_times.append(travel_times[i][j])

    return {"distances": flat_distances, "travelTimes": flat_travel_times}


def get_routes(
    start_lat: float = 30.59131044658496,
    start_lon: float = -96.33063373003323,
    end_lat: float = 30.61830965242742,
    end_lon: float = -96.3236700235634,
):
    """
    Find routes between two geographical points.

    https://valhalla1.openstreetmap.de/optimized_route?json={%22locations%22:[{%22lat%22:40.736072,%22lon%22:-73.856277,%22type%22:%22break%22},{%22lat%22:40.713696,%22lon%22:-73.757229,%22type%22:%22break%22}],%22costing%22:%22auto%22}

    Examples:
        uv run destiny routes
    """
    result = find_routes_between_points(
        start=(start_lat, start_lon),
        end=(end_lat, end_lon),
    )
    locations = convert_overpass_to_locations(result)
    travel_matrix = create_travel_matrix(locations)
    routing_matrix = {
        "name": "normal_car",
        "distances": travel_matrix["distances"],
        "travelTimes": travel_matrix["travelTimes"],
    }

    with open("routing_matrix.json", "w") as f:
        json.dump(routing_matrix, f, indent=4)
