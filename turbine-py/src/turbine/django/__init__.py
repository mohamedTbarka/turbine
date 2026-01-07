"""Django integration for Turbine task queue."""

from turbine.django.app import TurbineConfig

__all__ = ["TurbineConfig"]

default_app_config = "turbine.django.TurbineConfig"
