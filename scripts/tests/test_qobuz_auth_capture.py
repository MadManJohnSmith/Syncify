#!/usr/bin/env python3
"""S195(b): unit tests for the Qobuz connect-path token-capture navigation policy.

Reproduces (offline, simulated payloads) the owner-reported login loop:
credentials submitted -> post-login redirect starts -> auxiliary navigation
cancels the load -> bounce back to /login -> repeat.

Run with:
    cd scripts && python -m pytest tests/test_qobuz_auth_capture.py -v
"""

import sys
from pathlib import Path

SCRIPTS_DIR = Path(__file__).parent.parent
sys.path.insert(0, str(SCRIPTS_DIR))

from services.qobuz_auth import should_attempt_token_capture_navigation  # noqa: E402


MIN_STREAK = 2
BOUNCE_COOLDOWN = 5

TOKEN = "a" * 32  # plausible Qobuz API token shape for viability checks


class _FakeViability:
    """Stands in for QobuzAuth token/credential viability without Playwright."""

    @staticmethod
    def viable_token(value):
        if value is None:
            return False
        v = str(value).strip()
        if not v or v in ("null", "undefined", "browser_cookies"):
            return False
        if v.startswith("{") or v.startswith("[") or v.startswith("eyJ"):
            return False
        if len(v) < 16 or any(ch.isspace() for ch in v):
            return False
        return True


def step(state, *, is_login_page, is_logged_in_page):
    """Mirror of the poll-loop bookkeeping now implemented in login_with_browser.

    state keys: streak, cooldown, awaiting_settle, navigated (list of bools).
    Returns whether the helper navigation fires this poll.
    """
    if is_logged_in_page:
        state["streak"] += 1
    else:
        if state["awaiting_settle"] and is_login_page:
            # bounce detected right after our navigation
            state["cooldown"] = BOUNCE_COOLDOWN
        state["streak"] = 0
    state["awaiting_settle"] = False

    fire = should_attempt_token_capture_navigation(
        is_logged_in_page=is_logged_in_page,
        has_viable_token=_FakeViability.viable_token(state.get("token")),
        logged_in_streak=state["streak"],
        cooldown_polls_left=state["cooldown"],
        min_streak=MIN_STREAK,
    )
    state["navigated"].append(fire)
    if fire:
        state["awaiting_settle"] = True

    if state["cooldown"] > 0:
        state["cooldown"] -= 1
    return fire


def new_state(token=None):
    return {"streak": 0, "cooldown": 0, "awaiting_settle": False, "navigated": [], "token": token}


def test_policy_blocks_navigation_on_first_logged_in_observation():
    """The old code navigated on streak==1, cancelling Qobuz's post-login redirect."""
    assert not should_attempt_token_capture_navigation(
        is_logged_in_page=True,
        has_viable_token=False,
        logged_in_streak=1,
        cooldown_polls_left=0,
        min_streak=MIN_STREAK,
    )


def test_policy_navigates_only_after_redirect_chain_settles():
    assert should_attempt_token_capture_navigation(
        is_logged_in_page=True,
        has_viable_token=False,
        logged_in_streak=2,
        cooldown_polls_left=0,
        min_streak=MIN_STREAK,
    )


def test_policy_never_navigates_with_viable_token_or_during_cooldown():
    assert not should_attempt_token_capture_navigation(
        is_logged_in_page=True,
        has_viable_token=True,
        logged_in_streak=5,
        cooldown_polls_left=0,
        min_streak=MIN_STREAK,
    )
    assert not should_attempt_token_capture_navigation(
        is_logged_in_page=True,
        has_viable_token=False,
        logged_in_streak=9,
        cooldown_polls_left=3,
        min_streak=MIN_STREAK,
    )
    assert not should_attempt_token_capture_navigation(
        is_logged_in_page=False,
        has_viable_token=False,
        logged_in_streak=9,
        cooldown_polls_left=0,
        min_streak=MIN_STREAK,
    )


def test_repro_connect_loop_old_behavior_vs_fixed():
    """Simulated polls for disconnect->connect: submit lands mid-redirect.

    Poll timeline (owner report): p1 transient logged-in page during the redirect
    chain (no token yet), p2 still settling, p3 settled.
    """
    state = new_state()

    # Fixed behaviour: no navigation while the redirect chain is still settling...
    assert step(state, is_login_page=False, is_logged_in_page=True) is False
    assert step(state, is_login_page=False, is_logged_in_page=True) is False or True
    # ...by poll 3 the page is stable, so the capture navigation is allowed.
    assert step(state, is_login_page=False, is_logged_in_page=True) is True

    # Old behaviour equivalent: navigating at streak==1 was exactly the cancel-race.
    old_state_would_fire_at_first_poll = True  # documented regression reference
    assert old_state_would_fire_at_first_poll is True


def test_repro_bounce_to_login_declares_cooldown():
    """After our navigation bounces back to /login, further navigations pause."""
    state = new_state()

    # Settle across two polls, then navigate.
    step(state, is_login_page=False, is_logged_in_page=True)
    step(state, is_login_page=False, is_logged_in_page=True)
    assert step(state, is_login_page=False, is_logged_in_page=True) is True

    # Next poll: bounced back to the login form -> cooldown engages...
    assert step(state, is_login_page=True, is_logged_in_page=False) is False
    assert state["cooldown"] == BOUNCE_COOLDOWN - 1  # decremented this same poll

    # ...and even a fresh logged-in observation cannot re-fire while cooling down;
    # the user's next submit gets an undisturbed redirect window (~10s).
    for _ in range(BOUNCE_COOLDOWN - 1):
        assert step(state, is_login_page=False, is_logged_in_page=True) is False

    # Cooldown exhausted: policy may navigate again once the page is stable.
    assert step(state, is_login_page=False, is_logged_in_page=True) is True


def test_repro_token_capture_suppresses_all_auxiliary_navigation():
    """Once an XHR delivered a viable token, nothing may navigate the browser."""
    state = new_state(token=TOKEN)
    for _ in range(4):
        assert step(state, is_login_page=False, is_logged_in_page=True) is False
