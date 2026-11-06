function Get-ProductionSharedConnection {
 param([Parameter(Mandatory=$true)][string]$Directory)
 $metadata=Get-Content -LiteralPath (Join-Path $Directory 'client.json') -Raw | ConvertFrom-Json
 $uri=[Uri]$metadata.url
 if(-not $uri.IsAbsoluteUri -or $uri.Scheme -ne 'https' -or $uri.UserInfo -or $uri.Query -or $uri.Fragment -or $uri.AbsolutePath -ne '/' -or $uri.IsLoopback){throw 'В общей папке указан некорректный сетевой адрес. Администратор должен заново создать папку запуска.'}
 if($metadata.variant -ne 'shared-folder-browser' -or $metadata.certificateSha256 -cnotmatch '^[0-9a-f]{64}$'){throw 'Некорректный комплект общей папки. Обратитесь к администратору.'}
 $path=Join-Path $Directory 'server.cer'
 $hash=(Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash.ToLowerInvariant()
 if($hash -cne $metadata.certificateSha256){throw 'Сертификат в общей папке не соответствует настройкам. Не подтверждайте подключение. Администратор должен обновить комплект целиком.'}
 $cert=New-Object Security.Cryptography.X509Certificates.X509Certificate2($path)
 if($cert.HasPrivateKey){throw 'В папке запуска не должно быть закрытого ключа сервера.'}
 if($cert.NotAfter -lt (Get-Date) -or $cert.NotBefore -gt (Get-Date)){throw 'Сертификат ещё не действует или истёк. Проверьте дату ПК и обратитесь к администратору.'}
 return @{Url=$uri.AbsoluteUri.TrimEnd('/');Certificate=$cert;Hash=$hash}
}

function Test-ProductionSharedTrust {
 param([Parameter(Mandatory=$true)]$Connection)
 $store=New-Object Security.Cryptography.X509Certificates.X509Store('Root','CurrentUser')
 try{
  $store.Open([Security.Cryptography.X509Certificates.OpenFlags]::ReadOnly)
  $matches=$store.Certificates.Find([Security.Cryptography.X509Certificates.X509FindType]::FindByThumbprint,$Connection.Certificate.Thumbprint,$false)
  return $matches.Count -gt 0
 }finally{$store.Close()}
}

function Confirm-ProductionSharedTrust {
 param([Parameter(Mandatory=$true)]$Connection)
 if(Test-ProductionSharedTrust -Connection $Connection){return $true}
 $message="Адрес программы:`r`n$($Connection.Url)`r`n`r`nSHA256 сертификата:`r`n$($Connection.Hash)`r`n`r`nСверьте отпечаток с администратором лично или по телефону. Разрешить подключение только после сверки?`r`n`r`nСертификат добавится в доверенные только для вашего пользователя Windows. Права администратора не требуются."
 if([Windows.Forms.MessageBox]::Show($message,'Первое подключение к производственному циклу','YesNo','Question',[Windows.Forms.MessageBoxDefaultButton]::Button2) -ne 'Yes'){return $false}
 $store=New-Object Security.Cryptography.X509Certificates.X509Store('Root','CurrentUser')
 try{
  $store.Open([Security.Cryptography.X509Certificates.OpenFlags]::ReadWrite)
  $store.Add($Connection.Certificate)
 }finally{$store.Close()}
 if(-not(Test-ProductionSharedTrust -Connection $Connection)){throw 'Windows не сохранила доверие сертификату. Передайте сообщение администратору организации.'}
 return $true
}

function Test-ProductionSharedServer {
 param([Parameter(Mandatory=$true)]$Connection)
 # Keep normal Windows chain/hostname checks. Never install a global certificate callback.
 [Net.ServicePointManager]::SecurityProtocol=[Net.ServicePointManager]::SecurityProtocol -bor [Net.SecurityProtocolType]::Tls12
 $request=[Net.HttpWebRequest]::Create($Connection.Url+'/health')
 $request.Method='GET';$request.AllowAutoRedirect=$false;$request.Timeout=8000;$request.ReadWriteTimeout=8000
 $request.Proxy=$null
 $response=$null;$reader=$null
 try{
  $response=$request.GetResponse()
  if([int]$response.StatusCode -ne 200){throw 'Сервер не подтвердил готовность.'}
  $sha=[Security.Cryptography.SHA256]::Create()
  try{$hash=([BitConverter]::ToString($sha.ComputeHash($request.ServicePoint.Certificate.GetRawCertData()))).Replace('-','').ToLowerInvariant()}finally{$sha.Dispose()}
  if($hash -cne $Connection.Hash){throw 'Сертификат работающего сервера отличается от комплекта. Администратор должен обновить папку запуска.'}
  $reader=New-Object IO.StreamReader($response.GetResponseStream())
  $health=$reader.ReadToEnd() | ConvertFrom-Json
  if($health.application -cne 'production-cycle' -or $health.ready -ne $true){throw 'По этому адресу не найдена готовая программа Производственный цикл.'}
 }catch{
  throw ("Не удалось подключиться к программе по адресу $($Connection.Url).`r`nПапка доступна, но сервер программы или его HTTPS-порт могут быть недоступны. Передайте это сообщение администратору.`r`n`r`nПричина: "+$_.Exception.Message)
 }finally{if($reader){$reader.Dispose()};if($response){$response.Dispose()}}
}
