from destiny import Destiny


def test_extract_route_nodes_single_step():
    destiny = Destiny()
    osrm_response = {
        "routes": [
            {
                "legs": [
                    {
                        "steps": [
                            {
                                "geometry": {
                                    "coordinates": [
                                        [-2.2661375097762497, 53.47326189182173],
                                        [-2.2650000000000000, 53.47400000000000],
                                    ]
                                }
                            }
                        ]
                    }
                ]
            }
        ]
    }

    nodes = destiny.extract_route_nodes(osrm_response)

    assert len(nodes) == 2
    assert nodes[0] == (53.47326189182173, -2.2661375097762497)
    assert nodes[1] == (53.47400000000000, -2.2650000000000000)


def test_extract_route_nodes_multiple_steps():
    destiny = Destiny()
    osrm_response = {
        "routes": [
            {
                "legs": [
                    {
                        "steps": [
                            {
                                "geometry": {
                                    "coordinates": [
                                        [-2.266, 53.473],
                                        [-2.265, 53.474],
                                    ]
                                }
                            },
                            {
                                "geometry": {
                                    "coordinates": [
                                        [-2.264, 53.475],
                                        [-2.263, 53.476],
                                    ]
                                }
                            },
                        ]
                    }
                ]
            }
        ]
    }

    nodes = destiny.extract_route_nodes(osrm_response)

    assert len(nodes) == 4
    assert nodes[0] == (53.473, -2.266)
    assert nodes[1] == (53.474, -2.265)
    assert nodes[2] == (53.475, -2.264)
    assert nodes[3] == (53.476, -2.263)


def test_extract_route_nodes_empty_coordinates():
    destiny = Destiny()
    osrm_response = {
        "routes": [{"legs": [{"steps": [{"geometry": {"coordinates": []}}]}]}]
    }

    nodes = destiny.extract_route_nodes(osrm_response)

    assert len(nodes) == 0
    assert nodes == []


def test_call_osrm_api():
    """
    Utilse the open OSRM demo server to test the API call functionality.
    Note that this server has usage limits and should not be used for production.
    """
    destiny = Destiny(platform=False)
    start_coords = [(53.47326189182173, -2.2661375097762497)]
    end_coords = [(53.479416707143784, -2.241307253259415)]

    responses = destiny.call_osrm_api(start_coords, end_coords)

    assert len(responses) == 1
    response = responses[0]
    assert "routes" in response
    assert len(response["routes"]) > 0


def test_vec_get_route():
    destiny = Destiny(platform=False)
    start_coords = [(53.47326189182173, -2.2661375097762497)]
    end_coords = [(53.479416707143784, -2.241307253259415)]

    routes = destiny.vec_get_route(start_coords, end_coords)

    assert len(routes) == 1
    route = routes[0]
    assert "distance" in route
    assert "duration" in route
    assert "geometry" in route
    assert "nodes" in route
    assert len(route["nodes"]) > 0
