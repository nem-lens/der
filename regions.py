# Copyright (c) 2026 Archie C. Chapman and contributors
# SPDX-License-Identifier: BSD-3-Clause
"""Australian postcodes to states, and states to NEM regions."""

# NEM regions in a fixed order (largest first), used for every chart.
REGIONS = ["NSW1", "QLD1", "VIC1", "SA1", "TAS1"]
REGION_NAMES = {"NSW1": "NSW + ACT", "QLD1": "QLD", "VIC1": "VIC", "SA1": "SA", "TAS1": "TAS"}

# The ACT is part of the NSW1 region. WA and NT are outside the NEM.
STATE_REGION = {"NSW": "NSW1", "ACT": "NSW1", "QLD": "QLD1", "VIC": "VIC1", "SA": "SA1", "TAS": "TAS1"}
STATES = ["NSW", "ACT", "QLD", "VIC", "SA", "TAS", "WA", "NT"]


def state_of(postcode: int) -> str | None:
    """Australia Post's postcode ranges (delivery and PO box)."""
    p = postcode
    if 200 <= p <= 299 or 2600 <= p <= 2618 or 2900 <= p <= 2920:
        return "ACT"
    if 1000 <= p <= 2999:
        return "NSW"
    if 3000 <= p <= 3999 or 8000 <= p <= 8999:
        return "VIC"
    if 4000 <= p <= 4999 or 9000 <= p <= 9999:
        return "QLD"
    if 5000 <= p <= 5999:
        return "SA"
    if 6000 <= p <= 6999:
        return "WA"
    if 7000 <= p <= 7999:
        return "TAS"
    if 800 <= p <= 999:
        return "NT"
    return None
