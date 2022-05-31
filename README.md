# tradovate-api
## A framework to write a blazing-fast and reliable futures trading app on the tokio.rs runtime.
The program uses twilio to notify the user of time-sensitive events and emails to report trivial events. There are 8 threads, each with its own purpose. 
RW locks are preferred because it is engineered so that only one thread ever writes to a specific lock, and any other can read it. This prevents us from missing any messages. The program is in a loop that attemps to reconnect on the event of any disconnections or timeouts, while keeping all the other data in memory.
```
tokio::select! {
  () = md_read_future => false,
  () = account_read_future => false,
  _ = md_write_future => true,
  _ = account_write_future => true,
  _ = program_close_future => true,
  _ = periodic_reporting => false,
  _ = access_token_renewal_future => false,
  _ = log_file_print => false,
  () = strategy_calculations => false,
};
```
1. md_read_future. Reads all incoming messages on the market data socket and writes to the Market Data RW Lock.
2. account_read_future. Reads all incoming messages on the account data socket and writes to the Account RW Lock.
3. md_write_future. Writes to the market data socket.
4. account_write_future. Writes to the account future.
5. program_close_future. Initiates a shutdown sequence when program receives ctrl+c.
6. periodic_reporting. Sends the user email , text messages with the current state.
7. access_token_renewal. Renews your access token so you are not left without trading capabilities.
8. log_file_print. Prints to the log file so other operations are not slowed by file open/writes.
9. strategy_calculation. Reads any of the other RW locks to calculate whatever you need to calculate. From here you can send order to be sent via account write.
# To Use:
1. Clone and create a credentials.rs file. This should contain the following:
```
pub const CID: i64 = 1;
pub const SECRET: &str = "your tradovate api secret";
pub const USERNAME: &str = "your tradovate username";
pub const PASSWORD: &str = "your tradovate password";

pub const TWILIO_NUMBER: &str = "your twilio phone number";
pub const ASID: &str = "your twilio asid";
pub const TWILIO_AT: &str = "your twilio access token";
pub const EMAIL_USERNAME: &str = "your email to send yourself alerts";
pub const EMAIL_PASSWORD: &str = "your email password";
```
2. Make sure you are in sim because it will send orders. I am not responsible for your use of the app.
3. The settings.rs file contains the trading settings. It is configured for a simple strategy that monitors the DOM and Time and Sales.

