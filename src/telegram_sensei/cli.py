from __future__ import annotations

import asyncio
import configparser
import csv
import os
import platform
import sys
from datetime import datetime
from getpass import getpass
from pathlib import Path

import requests
from dotenv import load_dotenv
from rich.align import Align
from rich.console import Console, Group
from rich.markup import escape
from rich.panel import Panel
from rich.progress import Progress
from rich.table import Table
from rich.text import Text
from telethon import TelegramClient, functions
from telethon.errors import FloodWaitError, SessionPasswordNeededError
from telethon.errors.rpcerrorlist import UsernameInvalidError, UsernameOccupiedError

from telegram_sensei import __version__

console = Console()
ROOT_DIR = Path.cwd()
SESSION_DIR = ROOT_DIR / "sessions"
EXPORT_DIR = ROOT_DIR / "exports"
CONFIG_FILE = ROOT_DIR / "config.ini"
APP_VERSION = f"Telegram Sensei Toolkit v{__version__}"
DEVICE_MODEL = "Telegram Sensei CLI"
SYSTEM_VERSION = f"Python {platform.python_version()}"


def clear_screen() -> None:
    os.system("cls" if os.name == "nt" else "clear")


def prompt_for_int(prompt_text: str, default: int = 0) -> int:
    value = console.input(prompt_text).strip()
    if not value:
        return default
    try:
        return int(value)
    except ValueError:
        console.print("[red]Invalid number. Using default.[/red]")
        return default


def network_info() -> dict[str, str]:
    try:
        response = requests.get("https://ipapi.co/json/", timeout=4)
        response.raise_for_status()
        payload = response.json()
        return {"IP": payload.get("ip", "N/A"), "City": payload.get("city", "N/A")}
    except requests.RequestException:
        return {"IP": "N/A", "City": "Offline"}


class TelegramToolkit:
    def __init__(self) -> None:
        load_dotenv(ROOT_DIR / ".env")
        self.api_id = os.getenv("TELEGRAM_API_ID", "").strip()
        self.api_hash = os.getenv("TELEGRAM_API_HASH", "").strip()
        self.clients: dict[str, TelegramClient] = {}
        self.me_cache = {}
        self.active_session: str | None = None
        self.config = configparser.ConfigParser()
        if CONFIG_FILE.exists():
            self.config.read(CONFIG_FILE)
        self.last_active_session = self.config.get("user_settings", "last_active_session", fallback=None)

    def ensure_credentials(self) -> bool:
        if self.api_id and self.api_hash:
            return True
        console.print(Panel("Missing Telegram API credentials. Create .env from .env.example and add TELEGRAM_API_ID and TELEGRAM_API_HASH.", border_style="red"))
        return False

    def save_config(self) -> None:
        if "user_settings" not in self.config:
            self.config.add_section("user_settings")
        if self.active_session:
            self.config.set("user_settings", "last_active_session", self.active_session)
        with CONFIG_FILE.open("w", encoding="utf-8") as handle:
            self.config.write(handle)

    def session_path(self, session_name: str) -> str:
        SESSION_DIR.mkdir(exist_ok=True)
        return str(SESSION_DIR / session_name)

    async def load_sessions(self) -> None:
        if not self.ensure_credentials():
            return
        SESSION_DIR.mkdir(exist_ok=True)
        for session_file in sorted(SESSION_DIR.glob("*.session")):
            session_name = session_file.stem
            client = self.new_client(session_name)
            try:
                await client.connect()
                if await client.is_user_authorized():
                    self.clients[session_name] = client
                    self.me_cache[session_name] = await client.get_me()
                    console.print(f"[green]Loaded session {escape(session_name)}.[/green]")
                else:
                    await client.disconnect()
            except Exception as exc:
                console.print(f"[red]Could not load {escape(session_name)}: {escape(str(exc))}[/red]")
        if self.clients:
            self.active_session = self.last_active_session if self.last_active_session in self.clients else next(iter(self.clients))

    def new_client(self, session_name: str) -> TelegramClient:
        return TelegramClient(
            self.session_path(session_name),
            int(self.api_id),
            self.api_hash,
            device_model=DEVICE_MODEL,
            system_version=SYSTEM_VERSION,
            app_version=APP_VERSION,
        )

    def active_client(self) -> TelegramClient | None:
        if not self.active_session or self.active_session not in self.clients:
            console.print("[red]No active account. Login or switch account first.[/red]")
            return None
        return self.clients[self.active_session]

    def active_details(self) -> dict[str, str]:
        if not self.active_session or self.active_session not in self.me_cache:
            return {"Name": "N/A", "Phone": "N/A", "User ID": "N/A", "Username": "N/A"}
        me = self.me_cache[self.active_session]
        name = " ".join(part for part in [me.first_name, me.last_name] if part) or "N/A"
        return {
            "Name": escape(name),
            "Phone": f"+{me.phone}" if me.phone else "N/A",
            "User ID": str(me.id),
            "Username": f"@{me.username}" if me.username else "N/A",
        }

    async def login(self) -> None:
        if not self.ensure_credentials():
            return
        phone = console.input("[bold]Phone number with country code: [/bold]").strip()
        session_name = phone.replace("+", "").replace(" ", "")
        if not session_name:
            return
        if session_name in self.clients:
            console.print("[yellow]That account is already loaded.[/yellow]")
            return
        client = self.new_client(session_name)
        try:
            await client.connect()
            code = await client.send_code_request(phone)
            await client.sign_in(phone, console.input("[bold]Login code: [/bold]"), phone_code_hash=code.phone_code_hash)
        except SessionPasswordNeededError:
            await client.sign_in(password=getpass("2FA password: "))
        except Exception as exc:
            await client.disconnect()
            console.print(f"[red]Login failed: {escape(str(exc))}[/red]")
            return
        me = await client.get_me()
        self.clients[session_name] = client
        self.me_cache[session_name] = me
        self.active_session = session_name
        self.save_config()
        console.print(f"[green]Logged in as {escape(me.first_name or session_name)}.[/green]")

    async def check_inbox(self) -> None:
        client = self.active_client()
        if not client:
            return
        dialogs = await client.get_dialogs(limit=20)
        table = Table(title="Recent Chats")
        table.add_column("#", justify="right")
        table.add_column("Chat", style="cyan")
        table.add_column("Unread", justify="right")
        table.add_column("Last Message", overflow="fold")
        for index, dialog in enumerate(dialogs, 1):
            text = dialog.message.text if dialog.message and dialog.message.text else "[media/action]"
            table.add_row(str(index), escape(dialog.name), str(dialog.unread_count), escape(text.replace("\n", " ")))
        console.print(table)
        choice = prompt_for_int("[bold]Open chat number, 0 to back: [/bold]", 0)
        if not 1 <= choice <= len(dialogs):
            return
        messages = await client.get_messages(dialogs[choice - 1].entity, limit=10)
        message_table = Table(title=f"Last 10 messages from {escape(dialogs[choice - 1].name)}")
        message_table.add_column("Time")
        message_table.add_column("Sender")
        message_table.add_column("Message", overflow="fold")
        for message in reversed(messages):
            sender = "Me" if message.out else getattr(message.sender, "first_name", "Unknown")
            message_table.add_row(message.date.strftime("%H:%M"), escape(sender or "Unknown"), escape(message.text or "[media/action]"))
        console.print(message_table)

    async def quick_reply(self) -> None:
        client = self.active_client()
        if not client:
            return
        dialogs = await client.get_dialogs(limit=15)
        table = Table(title="Reply Targets")
        table.add_column("#", justify="right")
        table.add_column("Chat", style="cyan")
        table.add_column("Unread", justify="right")
        for index, dialog in enumerate(dialogs, 1):
            table.add_row(str(index), escape(dialog.name), str(dialog.unread_count))
        console.print(table)
        choice = prompt_for_int("[bold]Reply to chat number, 0 to back: [/bold]", 0)
        if not 1 <= choice <= len(dialogs):
            return
        message = console.input("[bold]Message: [/bold]").strip()
        if message:
            await client.send_message(dialogs[choice - 1].entity, message)
            console.print("[green]Reply sent.[/green]")

    async def export_chat(self) -> None:
        client = self.active_client()
        if not client:
            return
        target = console.input("[bold]Chat username, link, or ID: [/bold]").strip()
        limit = prompt_for_int("[bold]Messages to export: [/bold]", 100)
        if not target or limit <= 0:
            return
        entity = await client.get_entity(target)
        EXPORT_DIR.mkdir(exist_ok=True)
        filename = EXPORT_DIR / f"chat_{getattr(entity, 'id', 'export')}.txt"
        count = 0
        with filename.open("w", encoding="utf-8") as handle:
            async for message in client.iter_messages(entity, limit=limit):
                sender = await message.get_sender()
                sender_name = getattr(sender, "first_name", "Unknown")
                handle.write(f"[{message.date:%Y-%m-%d %H:%M}] {sender_name}: {message.text or '[media]'}\n")
                count += 1
        console.print(f"[green]Exported {count} messages to {filename}.[/green]")

    async def export_members(self) -> None:
        client = self.active_client()
        if not client:
            return
        target = console.input("[bold]Group/channel username, link, or ID: [/bold]").strip()
        limit = prompt_for_int("[bold]Members to export, 0 for all: [/bold]", 0)
        if not target:
            return
        entity = await client.get_entity(target)
        EXPORT_DIR.mkdir(exist_ok=True)
        filename = EXPORT_DIR / f"members_{getattr(entity, 'id', 'export')}.csv"
        count = 0
        with filename.open("w", encoding="utf-8", newline="") as handle:
            writer = csv.writer(handle)
            writer.writerow(["user_id", "username", "first_name", "last_name", "phone"])
            with Progress() as progress:
                task = progress.add_task("Exporting members", total=None)
                async for user in client.iter_participants(entity, limit=limit or None):
                    writer.writerow([user.id, user.username, user.first_name, user.last_name, user.phone])
                    count += 1
                    progress.update(task, description=f"Exported {count} members")
                    await asyncio.sleep(0.05)
        console.print(f"[green]Exported {count} members to {filename}.[/green]")

    async def manage_profile(self) -> None:
        client = self.active_client()
        if not client:
            return
        console.print(Panel("1. Change name\n2. Change bio\n3. Change username\n4. Upload profile picture", title="Profile"))
        choice = console.input("[bold]Select: [/bold]").strip()
        try:
            if choice == "1":
                await client(functions.account.UpdateProfileRequest(first_name=console.input("First name: "), last_name=console.input("Last name: ")))
            elif choice == "2":
                await client(functions.account.UpdateProfileRequest(about=console.input("Bio: ")[:70]))
            elif choice == "3":
                await client(functions.account.UpdateUsernameRequest(console.input("Username: ").strip()))
            elif choice == "4":
                path = Path(console.input("Photo path: ").strip()).expanduser()
                if not path.exists():
                    console.print("[red]File not found.[/red]")
                    return
                await client(functions.photos.UploadProfilePhotoRequest(file=await client.upload_file(path)))
            else:
                return
            console.print("[green]Profile updated.[/green]")
        except (UsernameInvalidError, UsernameOccupiedError) as exc:
            console.print(f"[red]Username error: {escape(str(exc))}[/red]")

    async def join_entity(self) -> None:
        client = self.active_client()
        if not client:
            return
        target = console.input("[bold]Public @username or t.me link: [/bold]").strip()
        if not target:
            return
        try:
            await client(functions.channels.JoinChannelRequest(target))
            console.print("[green]Joined successfully.[/green]")
        except FloodWaitError as exc:
            console.print(f"[red]Telegram rate limited this action. Wait {exc.seconds}s.[/red]")

    async def switch_account(self) -> None:
        if len(self.clients) < 2:
            console.print("[yellow]Login at least two accounts first.[/yellow]")
            return
        names = list(self.clients)
        for index, name in enumerate(names, 1):
            marker = " [green](active)[/green]" if name == self.active_session else ""
            console.print(f"{index}. {escape(name)}{marker}")
        choice = prompt_for_int("[bold]Select account: [/bold]", 0)
        if 1 <= choice <= len(names):
            self.active_session = names[choice - 1]
            self.save_config()

    async def delete_session(self) -> None:
        if not self.clients:
            console.print("[yellow]No sessions loaded.[/yellow]")
            return
        names = list(self.clients)
        for index, name in enumerate(names, 1):
            console.print(f"{index}. {escape(name)}")
        choice = prompt_for_int("[bold]Delete account number, 0 to cancel: [/bold]", 0)
        if not 1 <= choice <= len(names):
            return
        session_name = names[choice - 1]
        if console.input("Type DELETE to confirm: ") != "DELETE":
            return
        client = self.clients.pop(session_name)
        if client.is_connected():
            await client.log_out()
        for path in SESSION_DIR.glob(f"{session_name}.session*"):
            path.unlink(missing_ok=True)
        self.active_session = next(iter(self.clients), None)
        self.save_config()

    async def account_status(self) -> None:
        table = Table(title="Accounts")
        table.add_column("Session")
        table.add_column("Username")
        table.add_column("Status")
        for name, client in self.clients.items():
            me = self.me_cache.get(name) or await client.get_me()
            table.add_row(escape(name), f"@{me.username}" if me.username else "N/A", "online" if client.is_connected() else "offline")
        console.print(table)

    async def close(self) -> None:
        for client in self.clients.values():
            if client.is_connected():
                await client.disconnect()


def draw_header(toolkit: TelegramToolkit) -> None:
    info = Table.grid(padding=(0, 2))
    info.add_column(style="cyan")
    info.add_column()
    info.add_row("Version", __version__)
    info.add_row("Python", f"{sys.version_info.major}.{sys.version_info.minor}.{sys.version_info.micro}")
    info.add_row("Date", datetime.now().strftime("%Y-%m-%d"))
    for key, value in network_info().items():
        info.add_row(key, value)
    active = Table.grid(padding=(0, 2))
    active.add_column(style="cyan")
    active.add_column()
    active.add_row("Sessions", str(len(toolkit.clients)))
    for key, value in toolkit.active_details().items():
        active.add_row(key, value)
    console.print(Panel(Group(Panel(info, title="System"), Panel(active, title="Active Account")), title="Telegram Sensei Toolkit", border_style="blue"))


def draw_menu() -> None:
    menu = """
1. Check inbox and read messages
2. Quick reply
3. Export group/channel members
4. Export chat history
5. Manage profile
6. Join group/channel
7. Login new account
8. Switch account
9. Delete session
10. Check account status
0. Exit
""".strip()
    console.print(Panel(Align.center(Text("Built for JubairSenseiDev", style="bold cyan")), border_style="magenta"))
    console.print(Panel(menu, title="Main Menu", border_style="cyan"))


async def main() -> None:
    toolkit = TelegramToolkit()
    await toolkit.load_sessions()
    actions = {
        "1": toolkit.check_inbox,
        "2": toolkit.quick_reply,
        "3": toolkit.export_members,
        "4": toolkit.export_chat,
        "5": toolkit.manage_profile,
        "6": toolkit.join_entity,
        "7": toolkit.login,
        "8": toolkit.switch_account,
        "9": toolkit.delete_session,
        "10": toolkit.account_status,
    }
    try:
        while True:
            clear_screen()
            draw_header(toolkit)
            draw_menu()
            choice = console.input("[bold]Select an option > [/bold]").strip()
            if choice == "0":
                break
            action = actions.get(choice)
            if action is None:
                console.print("[red]Invalid option.[/red]")
                await asyncio.sleep(1)
                continue
            try:
                await action()
            except Exception as exc:
                console.print(f"[red]Action failed: {escape(str(exc))}[/red]")
            console.input("[dim]Press Enter to continue.[/dim]")
    finally:
        await toolkit.close()


def run() -> None:
    try:
        asyncio.run(main())
    except KeyboardInterrupt:
        console.print("\n[yellow]Exited.[/yellow]")
