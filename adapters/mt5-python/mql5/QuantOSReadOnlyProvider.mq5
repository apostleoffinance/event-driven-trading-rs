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
      return false;

   uchar bytes[];
   int count = StringToCharArray(value, bytes, 0, WHOLE_ARRAY, CP_UTF8);
   if(count <= 1)
      return false;
   int sent = SocketSend(g_socket, bytes, (uint)(count - 1));
   return (sent == count - 1);
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
   g_socket = SocketCreate();
   if(g_socket < 0)
      return false;

   if(!SocketConnect(g_socket, InpHost, InpPort, 1000))
   {
      CloseSocket();
      return false;
   }

   if(!SendText("HELLO\t" + InpSharedToken + "\n"))
   {
      CloseSocket();
      return false;
   }
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

   return "{\"symbol\":" + JsonString(symbol) +
          ",\"bid\":" + JsonNumber(tick.bid, 16) +
          ",\"ask\":" + JsonNumber(tick.ask, 16) +
          ",\"time\":" + IntegerToString((long)tick.time) + "}";
}

void Respond(const bool ok, const string data)
{
   string response;
   if(ok)
      response = "{\"ok\":true,\"data\":" + data + "}\n";
   else
      response = "{\"ok\":false,\"error\":\"read-only request unavailable\"}\n";

   if(!SendText(response))
      CloseSocket();
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
   if(g_socket < 0 || !SocketIsConnected(g_socket) || !SocketIsReadable(g_socket))
      return;

   uchar bytes[];
   ArrayResize(bytes, 8192);
   int received = SocketRead(g_socket, bytes, (uint)ArraySize(bytes), 50);
   if(received <= 0)
   {
      if(!SocketIsConnected(g_socket))
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
   if(g_socket < 0 || !SocketIsConnected(g_socket))
   {
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
