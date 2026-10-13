"""Config, options and reauth flows for Aquila.

Implements ``docs/spec/home-assistant.md`` §7: a single-field user step
validated against ``GET /api/v1/health`` (the entry unique id is the core's
``instance_id``), an options flow, and a reauth step for the optional token.
"""

from __future__ import annotations

import logging
from typing import Any

import voluptuous as vol

from homeassistant.config_entries import ConfigEntry, ConfigFlow, OptionsFlow
from homeassistant.core import HomeAssistant
from homeassistant.helpers.aiohttp_client import async_get_clientsession
from homeassistant.helpers.selector import (
    BooleanSelector,
    NumberSelector,
    NumberSelectorConfig,
    NumberSelectorMode,
    TextSelector,
    TextSelectorConfig,
    TextSelectorType,
)

from .api import (
    AquilaApiClient,
    AquilaAuthError,
    AquilaConnectionError,
    AquilaError,
    AquilaVersionError,
)
from .const import (
    CONF_EXPOSE_FLOORS,
    CONF_EXPOSE_ROOM_VITALS,
    CONF_TOKEN,
    CONF_UPDATE_INTERVAL,
    CONF_URL,
    DEFAULT_EXPOSE_FLOORS,
    DEFAULT_EXPOSE_ROOM_VITALS,
    DEFAULT_UPDATE_INTERVAL,
    DEFAULT_URL,
    DOMAIN,
    HEALTH_INSTANCE_ID,
    MAX_UPDATE_INTERVAL,
    MIN_UPDATE_INTERVAL,
)

_LOGGER = logging.getLogger(__name__)

CONFIG_SCHEMA = vol.Schema(
    {
        vol.Required(CONF_URL, default=DEFAULT_URL): TextSelector(
            TextSelectorConfig(type=TextSelectorType.URL)
        ),
        vol.Optional(CONF_TOKEN): TextSelector(
            TextSelectorConfig(type=TextSelectorType.PASSWORD)
        ),
    }
)

REAUTH_SCHEMA = vol.Schema(
    {
        vol.Optional(CONF_TOKEN): TextSelector(
            TextSelectorConfig(type=TextSelectorType.PASSWORD)
        ),
    }
)


class AquilaConfigFlow(ConfigFlow, domain=DOMAIN):
    """Enforce one config entry per Aquila core instance."""

    VERSION = 1

    async def async_step_user(
        self, user_input: dict[str, Any] | None = None
    ) -> dict[str, Any]:
        """Handle the initial setup step (Core URL, optional token)."""
        errors: dict[str, str] = {}

        if user_input is not None:
            url = user_input[CONF_URL].rstrip("/")
            token = user_input.get(CONF_TOKEN) or None
            client = AquilaApiClient(
                async_get_clientsession(self.hass), url, token
            )
            try:
                health = await client.async_validate()
            except AquilaAuthError:
                errors["base"] = "invalid_auth"
            except AquilaVersionError:
                errors["base"] = "unsupported_version"
            except AquilaConnectionError:
                errors["base"] = "cannot_connect"
            except AquilaError:
                errors["base"] = "unknown"
            else:
                instance_id = health.get(HEALTH_INSTANCE_ID)
                if not instance_id:
                    errors["base"] = "invalid_response"
                else:
                    await self.async_set_unique_id(str(instance_id))
                    self._abort_if_unique_id_configured()
                    return self.async_create_entry(
                        title="Aquila",
                        data={CONF_URL: url, CONF_TOKEN: token},
                    )

        return self.async_show_form(
            step_id="user",
            data_schema=CONFIG_SCHEMA,
            errors=errors,
        )

    async def async_step_reauth(
        self, entry_data: dict[str, Any]
    ) -> dict[str, Any]:
        """Start reauth (only relevant when a token is enabled)."""
        return await self.async_step_reauth_confirm()

    async def async_step_reauth_confirm(
        self, user_input: dict[str, Any] | None = None
    ) -> dict[str, Any]:
        """Replace the token and reload the entry."""
        errors: dict[str, str] = {}
        entry = self._get_reauth_entry()

        if user_input is not None:
            token = user_input.get(CONF_TOKEN) or None
            client = AquilaApiClient(
                async_get_clientsession(self.hass), entry.data[CONF_URL], token
            )
            try:
                await client.async_validate()
            except AquilaAuthError:
                errors["base"] = "invalid_auth"
            except AquilaVersionError:
                errors["base"] = "unsupported_version"
            except AquilaConnectionError:
                errors["base"] = "cannot_connect"
            except AquilaError:
                errors["base"] = "unknown"
            else:
                self.hass.config_entries.async_update_entry(
                    entry, data={**entry.data, CONF_TOKEN: token}
                )
                await self.hass.config_entries.async_reload(entry.entry_id)
                return self.async_abort(reason="reauth_successful")

        return self.async_show_form(
            step_id="reauth_confirm", data_schema=REAUTH_SCHEMA, errors=errors
        )

    @staticmethod
    def async_get_options_flow(config_entry: ConfigEntry) -> "AquilaOptionsFlow":
        """Return the options flow."""
        return AquilaOptionsFlow()


class AquilaOptionsFlow(OptionsFlow):
    """Options: reconciliation cadence and entity exposure."""

    async def async_step_init(
        self, user_input: dict[str, Any] | None = None
    ) -> dict[str, Any]:
        """Manage the options."""
        entry = self.config_entry

        if user_input is not None:
            return self.async_create_entry(title="", data=user_input)

        current = {**entry.options}
        schema = vol.Schema(
            {
                vol.Optional(
                    CONF_UPDATE_INTERVAL,
                    default=current.get(
                        CONF_UPDATE_INTERVAL, DEFAULT_UPDATE_INTERVAL
                    ),
                ): NumberSelector(
                    NumberSelectorConfig(
                        min=MIN_UPDATE_INTERVAL,
                        max=MAX_UPDATE_INTERVAL,
                        step=1,
                        mode=NumberSelectorMode.BOX,
                        unit_of_measurement="s",
                    )
                ),
                vol.Optional(
                    CONF_EXPOSE_FLOORS,
                    default=current.get(
                        CONF_EXPOSE_FLOORS, DEFAULT_EXPOSE_FLOORS
                    ),
                ): BooleanSelector(),
                vol.Optional(
                    CONF_EXPOSE_ROOM_VITALS,
                    default=current.get(
                        CONF_EXPOSE_ROOM_VITALS, DEFAULT_EXPOSE_ROOM_VITALS
                    ),
                ): BooleanSelector(),
            }
        )
        return self.async_show_form(step_id="init", data_schema=schema)


async def async_migrate_entry(hass: HomeAssistant, entry: ConfigEntry) -> bool:
    """Migrate a config entry across schema bumps (home-assistant.md §7).

    The core owns its own ``/data`` schema migrations on start; this only
    handles the component's config-entry version. v1 has no prior versions, so
    there is nothing to migrate yet.
    """
    if entry.version > AquilaConfigFlow.VERSION:
        # A downgrade is not supported.
        _LOGGER.error(
            "Cannot migrate Aquila entry from version %s to %s",
            entry.version,
            AquilaConfigFlow.VERSION,
        )
        return False
    return True
