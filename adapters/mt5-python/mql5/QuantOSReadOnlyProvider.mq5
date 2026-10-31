//+------------------------------------------------------------------+
//| QuantOSReadOnlyProvider.mq5                                      |
//| Read-only loopback socket adapter for Quant OS on macOS MT5.     |
//| No order-send, order-modify, or order-delete functions exist.    |
//+------------------------------------------------------------------+
#property strict
#property version   "1.00"
#property description "Read-only Quant OS terminal data provider"

input string InpHost = "127.0.0.1";
input ushort InpPort = 8765;
input string InpSharedToken = "";
input string InpAllowedSymbols = "EURUSD";

int g_socket = -1;
string g_rx_buffer = "";
string g_symbols[];

string JsonEscape(string value)
{
   StringReplace(value, "\\", "\\\\");
   StringReplace(value, "\"", "\\\"");
   StringReplace(value, "\r", "\\r");
   StringReplace(value, "\n", "\\n");
   StringReplace(value, "\t", "\\t");
   return value;
}

string JsonString(string value)
{
   return "\"" + JsonEscape(value) + "\"";
}

string JsonNumber(double value, int digits = 8)
{
   return DoubleToString(value, digits);
}

bool SendText(const string value)
{
   if(g_socket < 0 || !SocketIsConnected(g_socket))
   {
      PrintFormat("QuantOS socket send skipped: socket disconnected (handle=%d)", g_socket);
      return false;
   }

   uchar bytes[];
   int count = StringToCharArray(value, bytes, 0, WHOLE_ARRAY, CP_UTF8);
   if(count <= 1)
   {
      Print("QuantOS socket send failed: UTF-8 frame encoded to zero bytes.");
      return false;
   }

   ResetLastError();
   int expected = count - 1;
   int sent = SocketSend(g_socket, bytes, (uint)expected);
   if(sent != expected)
   {
      int error = GetLastError();
      PrintFormat("QuantOS socket send failed: sent=%d expected=%d error=%d",
                  sent, expected, error);
      return false;
   }
   return true;
}

void CloseSocket()
{
   if(g_socket >= 0)
   {
      SocketClose(g_socket);
      g_socket = -1;
   }
   g_rx_buffer = "";
}

bool ConnectProvider()
{
   if(g_socket >= 0 && SocketIsConnected(g_socket))
      return true;

   CloseSocket();
   ResetLastError();
   g_socket = SocketCreate();
   if(g_socket < 0)
   {
      PrintFormat("QuantOS socket create failed: error=%d", GetLastError());
      return false;
   }

   ResetLastError();
   if(!SocketConnect(g_socket, InpHost, InpPort, 1000))
   {
      int error = GetLastError();
      PrintFormat("QuantOS socket connect failed: host=%s port=%d error=%d",
                  InpHost, InpPort, error);
      CloseSocket();
      return false;
   }

   PrintFormat("QuantOS socket connected to %s:%d; sending handshake.", InpHost, InpPort);
   if(!SendText("HELLO\t" + InpSharedToken + "\n"))
   {
      Print("QuantOS socket handshake send failed; reconnecting.");
      CloseSocket();
      return false;
   }
   Print("QuantOS socket handshake sent.");
   return true;
}

bool IsAllowedSymbol(const string symbol)
{
   for(int i = 0; i < ArraySize(g_symbols); i++)
   {
      if(g_symbols[i] == symbol)
         return true;
   }
   return false;
}

string AccountJson()
{
   if(!TerminalInfoInteger(TERMINAL_CONNECTED))
      return "";

   long login = AccountInfoInteger(ACCOUNT_LOGIN);
   string server = AccountInfoString(ACCOUNT_SERVER);
   string currency = AccountInfoString(ACCOUNT_CURRENCY);
   if(login <= 0 || server == "" || currency == "")
      return "";

   return "{\"login\":" + IntegerToString(login) +
          ",\"server\":" + JsonString(server) +
          ",\"currency\":" + JsonString(currency) +
          ",\"balance\":" + JsonNumber(AccountInfoDouble(ACCOUNT_BALANCE), 8) +
          ",\"equity\":" + JsonNumber(AccountInfoDouble(ACCOUNT_EQUITY), 8) +
          ",\"margin\":" + JsonNumber(AccountInfoDouble(ACCOUNT_MARGIN), 8) +
          ",\"margin_free\":" + JsonNumber(AccountInfoDouble(ACCOUNT_MARGIN_FREE), 8) + "}";
}

string OrdersJson()
{
   string result = "[";
   bool first = true;
   int total = OrdersTotal();

   for(int i = 0; i < total; i++)
   {
      ulong ticket = OrderGetTicket(i);
      if(ticket == 0)
         continue;

      long type = OrderGetInteger(ORDER_TYPE);
      long state = OrderGetInteger(ORDER_STATE);
      string symbol = OrderGetString(ORDER_SYMBOL);
      double volume = OrderGetDouble(ORDER_VOLUME_CURRENT);

      if(!first)
         result += ",";
      first = false;
      result += "{\"ticket\":" + IntegerToString((long)ticket) +
                ",\"type\":" + IntegerToString(type) +
                ",\"symbol\":" + JsonString(symbol) +
                ",\"volume_current\":" + JsonNumber(volume, 8) +
                ",\"state\":" + IntegerToString(state) + "}";
   }
   return result + "]";
}

string PositionsJson()
{
   string result = "[";
   bool first = true;
   int total = PositionsTotal();

   for(int i = 0; i < total; i++)
   {
      ulong ticket = PositionGetTicket(i);
      if(ticket == 0)
         continue;

      long type = PositionGetInteger(POSITION_TYPE);
      string symbol = PositionGetString(POSITION_SYMBOL);
      double volume = PositionGetDouble(POSITION_VOLUME);
      double price_open = PositionGetDouble(POSITION_PRICE_OPEN);
      double profit = PositionGetDouble(POSITION_PROFIT);

      if(!first)
         result += ",";
      first = false;
      result += "{\"ticket\":" + IntegerToString((long)ticket) +
                ",\"type\":" + IntegerToString(type) +
                ",\"symbol\":" + JsonString(symbol) +
                ",\"volume\":" + JsonNumber(volume, 8) +
                ",\"price_open\":" + JsonNumber(price_open, 16) +
                ",\"profit\":" + JsonNumber(profit, 8) + "}";
   }
   return result + "]";
}

string SymbolInfoJson(const string symbol)
{
   if(!IsAllowedSymbol(symbol) || !SymbolSelect(symbol, true))
      return "";

   long digits = 0;
   long trade_mode = 0;
   if(!SymbolInfoInteger(symbol, SYMBOL_DIGITS, digits) ||
      !SymbolInfoInteger(symbol, SYMBOL_TRADE_MODE, trade_mode))
      return "";

   string base_currency = SymbolInfoString(symbol, SYMBOL_CURRENCY_BASE);
   string quote_currency = SymbolInfoString(symbol, SYMBOL_CURRENCY_PROFIT);
   if(base_currency == "" || quote_currency == "")
      return "";

   return "{\"name\":" + JsonString(symbol) +
          ",\"currency_base\":" + JsonString(base_currency) +
          ",\"currency_profit\":" + JsonString(quote_currency) +
          ",\"digits\":" + IntegerToString(digits) +
          ",\"point\":" + JsonNumber(SymbolInfoDouble(symbol, SYMBOL_POINT), 16) +
          ",\"trade_tick_size\":" + JsonNumber(SymbolInfoDouble(symbol, SYMBOL_TRADE_TICK_SIZE), 16) +
          ",\"trade_tick_value\":" + JsonNumber(SymbolInfoDouble(symbol, SYMBOL_TRADE_TICK_VALUE), 8) +
          ",\"trade_contract_size\":" + JsonNumber(SymbolInfoDouble(symbol, SYMBOL_TRADE_CONTRACT_SIZE), 8) +
          ",\"volume_min\":" + JsonNumber(SymbolInfoDouble(symbol, SYMBOL_VOLUME_MIN), 8) +
          ",\"volume_max\":" + JsonNumber(SymbolInfoDouble(symbol, SYMBOL_VOLUME_MAX), 8) +
          ",\"volume_step\":" + JsonNumber(SymbolInfoDouble(symbol, SYMBOL_VOLUME_STEP), 8) +
          ",\"trade_mode\":" + IntegerToString(trade_mode) + "}";
}

string TickJson(const string symbol)
{
   if(!IsAllowedSymbol(symbol) || !SymbolSelect(symbol, true))
      return "";

   MqlTick tick;
   if(!SymbolInfoTick(symbol, tick) || tick.time <= 0)
      return "";

   // Diagnostic only: compare the quote epoch with MT5's clock functions.
   // This logs no account credentials, tokens, prices, or other sensitive data.
   long tick_epoch = (long)tick.time;
   long utc_epoch = (long)TimeGMT();
   long local_epoch = (long)TimeLocal();
   long current_epoch = (long)TimeCurrent();
   long server_epoch = (long)TimeTradeServer();
   int gmt_offset = TimeGMTOffset();
   PrintFormat("QuantOS time diagnostic: symbol=%s tick_epoch=%I64d utc_epoch=%I64d local_epoch=%I64d current_epoch=%I64d server_epoch=%I64d gmt_offset_seconds=%d tick_minus_utc_seconds=%I64d",
               symbol, tick_epoch, utc_epoch, local_epoch, current_epoch, server_epoch,
               gmt_offset, tick_epoch - utc_epoch);

   // TimeCurrent is only the last received quote time and can freeze while
   // the market is closed. TimeTradeServer advances using the terminal's
   // synchronized host clock and is the reference for normalizing quote time.
   return "{\"symbol\":" + JsonString(symbol) +
          ",\"bid\":" + JsonNumber(tick.bid, 16) +
          ",\"ask\":" + JsonNumber(tick.ask, 16) +
          ",\"time\":" + IntegerToString((long)tick.time) +
          ",\"server_time\":" + IntegerToString((long)TimeTradeServer()) + "}";
}

void Respond(const bool ok, const string data)
{
   string response;
   if(ok)
      response = "{\"ok\":true,\"data\":" + data + "}\n";
   else
      response = "{\"ok\":false,\"error\":\"read-only request unavailable\"}\n";

   if(!SendText(response))
   {
      PrintFormat("QuantOS socket response send failed: ok=%s", ok ? "true" : "false");
      CloseSocket();
   }
}

void ProcessRequest(const string line)
{
   string parts[];
   int count = StringSplit(line, '\t', parts);
   if(count < 1 || count > 2)
   {
      Respond(false, "null");
      return;
   }

   string command = parts[0];
   string argument = (count == 2 ? parts[1] : "");

   if(command == "PING" && count == 1)
   {
      string account = AccountJson();
      if(account == "")
      {
         Respond(true, "{\"connected\":false,\"login\":0,\"server\":\"\"}");
         return;
      }
      long login = AccountInfoInteger(ACCOUNT_LOGIN);
      string server = AccountInfoString(ACCOUNT_SERVER);
      Respond(true, "{\"connected\":true,\"login\":" + IntegerToString(login) +
                    ",\"server\":" + JsonString(server) + "}");
      return;
   }

   if(command == "ACCOUNT" && count == 1)
   {
      string data = AccountJson();
      Respond(data != "", data);
      return;
   }
   if(command == "ORDERS" && count == 1)
   {
      Respond(TerminalInfoInteger(TERMINAL_CONNECTED), OrdersJson());
      return;
   }
   if(command == "POSITIONS" && count == 1)
   {
      Respond(TerminalInfoInteger(TERMINAL_CONNECTED), PositionsJson());
      return;
   }
   if(command == "SELECT_SYMBOL" && count == 2)
   {
      bool selected = IsAllowedSymbol(argument) && SymbolSelect(argument, true);
      Respond(true, selected ? "true" : "false");
      return;
   }
   if(command == "SYMBOL_INFO" && count == 2)
   {
      string data = SymbolInfoJson(argument);
      Respond(data != "", data);
      return;
   }
   if(command == "TICK" && count == 2)
   {
      string data = TickJson(argument);
      Respond(data != "", data);
      return;
   }

   Respond(false, "null");
}

void DrainSocket()
{
   if(g_socket < 0)
      return;

   // SocketIsReadable returns the number of bytes available. Pass that exact
   // count to SocketRead instead of asking it to read an unrelated 8 KiB.
   // This follows the MQL5 socket API pattern and avoids platform-specific
   // read failures with Wine/CrossOver MT5 builds.
   ResetLastError();
   uint available = SocketIsReadable(g_socket);
   int readable_error = GetLastError();
   if(available == 0)
   {
      if(readable_error != 0)
      {
         PrintFormat("QuantOS socket readability check failed: error=%d", readable_error);
         CloseSocket();
      }
      return;
   }

   uchar bytes[];
   ResetLastError();
   int received = SocketRead(g_socket, bytes, available, 50);
   int read_error = GetLastError();
   if(received <= 0)
   {
      // A timeout may return -1 even while the connection remains alive.
      // Only discard the socket when MQL5 reports a disconnected peer or an
      // actual socket I/O error; keep the diagnostic free of payload/secrets.
      bool still_connected = SocketIsConnected(g_socket);
      if(read_error != 0 || !still_connected)
         PrintFormat("QuantOS socket read failed: available=%u received=%d error=%d connected=%s",
                     available, received, read_error, still_connected ? "true" : "false");
      if(!still_connected || read_error == 5273)
         CloseSocket();
      return;
   }

   g_rx_buffer += CharArrayToString(bytes, 0, received, CP_UTF8);
   if(StringLen(g_rx_buffer) > 65536)
   {
      CloseSocket();
      return;
   }

   while(true)
   {
      int newline = StringFind(g_rx_buffer, "\n");
      if(newline < 0)
         break;
      string line = StringSubstr(g_rx_buffer, 0, newline);
      g_rx_buffer = StringSubstr(g_rx_buffer, newline + 1);
      if(StringLen(line) > 1024)
      {
         CloseSocket();
         return;
      }
      PrintFormat("QuantOS socket request received: command=%s", StringSubstr(line, 0, StringFind(line, "\t") >= 0 ? StringFind(line, "\t") : StringLen(line)));
      ProcessRequest(line);
      if(g_socket < 0)
         return;
   }
}

int OnInit()
{
   if(InpHost != "127.0.0.1" || InpPort < 1024 || InpSharedToken == "" ||
      StringLen(InpSharedToken) < 32)
   {
      Print("QuantOS provider blocked: loopback host, valid port, and 32+ character shared token are required.");
      return INIT_PARAMETERS_INCORRECT;
   }

   int count = StringSplit(InpAllowedSymbols, ',', g_symbols);
   if(count < 1 || count > 64)
   {
      Print("QuantOS provider blocked: configure 1-64 exact broker symbols.");
      return INIT_PARAMETERS_INCORRECT;
   }
   for(int i = 0; i < count; i++)
   {
      StringTrimLeft(g_symbols[i]);
      StringTrimRight(g_symbols[i]);
      if(g_symbols[i] == "")
      {
         Print("QuantOS provider blocked: empty symbol in allowlist.");
         return INIT_PARAMETERS_INCORRECT;
      }
      for(int j = 0; j < i; j++)
      {
         if(g_symbols[j] == g_symbols[i])
         {
            Print("QuantOS provider blocked: duplicate symbol in allowlist.");
            return INIT_PARAMETERS_INCORRECT;
         }
      }
   }

   EventSetMillisecondTimer(100);
   Print("QuantOS read-only provider initialized. Order execution is not supported.");
   return INIT_SUCCEEDED;
}

void OnTimer()
{
   if(g_socket < 0)
   {
      ConnectProvider();
      return;
   }
   if(!SocketIsConnected(g_socket))
   {
      Print("QuantOS socket peer disconnected; reconnecting.");
      CloseSocket();
      ConnectProvider();
      return;
   }
   DrainSocket();
}

void OnDeinit(const int reason)
{
   EventKillTimer();
   CloseSocket();
}
