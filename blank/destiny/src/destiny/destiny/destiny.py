import os
import time

import haversine
import requests
from rich.progress import (
    BarColumn,
    Progress,
    TextColumn,
    TimeElapsedColumn,
    TimeRemainingColumn,
)


class Destiny:
    """Destiny class to handle routing using OSRM API."""

    def __init__(self, platform: bool = True):
        """
        Initialize Destiny with platform settings.

        Args:
            platform (bool): Whether to use the platform-specific OSRM endpoint.
            You will have to deploy OSRM to your tenant for heavy usage.
        """
        self.platform = platform
        self.tenant = os.getenv("TENANT", "example") if platform else None
        self.base_url = (
            f"https://osrm-instance-v1-prod-{self.tenant}.service.nimbus.example/route/v1/driving/"
            if platform
            else "http://router.project-osrm.org/route/v1/driving/"
        )

    def call_osrm_api(
        self,
        start_coords: list[tuple],
        end_coords: list[tuple],
    ) -> list[dict]:
        """
        OSRM offer a free public API for routing providing the useage is not heavy
        or more than one request a second: https://map.project-osrm.org/about.html.

        Therefore, the intention is to use this when we only require a small number of
        one off queries. If you need heavy or constant useage then deploy OSRM to your
        tenant and use that endpoint instead.

        Args:
            start_coords (list[tuple]): List of (lat, lon) tuples for start points
            end_coords (list[tuple]): List of (lat, lon) tuples for end points
        """
        progress = Progress(
            TextColumn("[progress.description]{task.description}"),
            BarColumn(),
            "[progress.percentage]{task.percentage:>3.0f}%",
            TimeElapsedColumn(),
            TimeRemainingColumn(),
        )

        responses = []
        with progress:
            task = progress.add_task("Calling OSRM API...", total=len(start_coords))
            for start_coord, end_coord in zip(start_coords, end_coords):
                coordinates = f"{start_coord[1]:.8f},{start_coord[0]:.8f};{end_coord[1]:.8f},{end_coord[0]:.8f}"
                url = f"{self.base_url}{coordinates}?steps=true&overview=full&geometries=geojson"

                response = requests.get(
                    url,
                    headers={
                        "Authorization": os.getenv("API_KEY", "")
                        if self.platform
                        else ""
                    },
                    timeout=10,
                )
                response.raise_for_status()
                responses.append(response.json())

                if not self.platform:
                    # Ensure we do not exceed 1 request per second
                    time.sleep(1.1)

                progress.advance(task)

        return responses

    def extract_route_nodes(self, osrm_response: dict) -> list[tuple]:
        steps = osrm_response["routes"][0]["legs"][0]["steps"]
        nodes = []
        for step in steps:
            for coord in step["geometry"]["coordinates"]:
                nodes.append((coord[1], coord[0]))  # (lat, lon)
        return nodes

    def vec_get_route(
        self,
        start_coords: list[tuple] = [(53.47326189182173, -2.2661375097762497)],
        end_coords: list[tuple] = [(53.479416707143784, -2.241307253259415)],
        distance_unit: str = "km",
        time_unit: str = "min",
    ):
        """
        This does not return what the origonal did, nor does it take the same input arugments.

        Rather the user is expected to handle further processing of either.
        """

        if time_unit not in ["sec", "min"]:
            raise ValueError("time_unit must be either 'sec' or 'min'")

        if distance_unit not in ["m", "km"]:
            raise ValueError("distance_unit must be either 'm' or 'km'")

        response = self.call_osrm_api(
            start_coords=start_coords,
            end_coords=end_coords,
        )

        response_morphated = []
        for resp in response:
            route_info = resp["routes"][0]
            distance = route_info["distance"]  # in meters
            duration = route_info["duration"]  # in seconds

            if distance_unit == "km":
                distance /= 1000  # convert to kilometers
            if time_unit == "min":
                duration /= 60  # convert to minutes

            route_dict = {
                "distance": distance,
                "duration": duration,
                "geometry": route_info["geometry"],
                "nodes": self.extract_route_nodes(resp),
            }

            response_morphated.append(route_dict)

        return response_morphated

    def compute_haversine_distance(
        self,
        start_coords: list[tuple],
        end_coords: list[tuple],
        unit: str = "km",
    ) -> list[float]:
        """
        Compute Haversine distance between start and end coordinates.

        Args:
            start_coords (list[tuple]): List of (lat, lon) tuples for start points
            end_coords (list[tuple]): List of (lat, lon) tuples for end points
            unit (str): Unit of distance ('km' or 'miles')
        Returns:
            list[float]: List of distances between start and end points
        """
        if unit not in ["km", "miles"]:
            raise ValueError("unit must be either 'km' or 'miles'")

        distances = []
        for start, end in zip(start_coords, end_coords):
            distance = haversine.haversine(start, end, unit=unit)
            distances.append(distance)

        return distances
